import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'
import {
  lbrAgenticCodingRequestToDraft,
  makeLbrAgenticCodingRequest,
} from '@openagentsinc/nip90'

import type { VerifiedPublicIdentityClaim } from './agent-owner-claim-routes'
import { makeForumRoutes } from './forum-routes'
import { makeFakeOpenAgentsHostedMdkClient } from './hosted-mdk-client'
import {
  makeOpenAgentsL402HmacSigningBoundary,
} from './l402-credential-service'

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

type ChallengeRow = Readonly<{
  action_kind: string
  actor_ref: string
  archived_at: string | null
  created_at: string
  expires_at: string
  id: string
  idempotency_key: string
  method: 'POST'
  path: string
  price_asset: 'credits' | 'sats' | 'usd'
  price_value: number
  public_projection_json: string
  recipient_actor_ref: string | null
  recipient_readiness_ref: string | null
  request_body_digest: string
  route_params_json: string
  spend_cap_asset: 'credits' | 'sats' | 'usd'
  spend_cap_value: number
  target_forum_id: string | null
  target_post_id: string | null
  target_topic_id: string | null
  mdk_provider_ref: string | null
  mdk_environment: 'production' | 'sandbox' | null
  mdk_sandbox: number | null
  mdk_implementation_state:
    | 'fake_provider_contract'
    | 'live_provider_configured'
    | 'missing_configuration'
    | null
  mdk_checkout_ref: string | null
  mdk_checkout_url_ref: string | null
  mdk_checkout_launch_path: string | null
  mdk_invoice_ref: string | null
  mdk_payment_hash_ref: string | null
  l402_credential_ref: string | null
  l402_replay_nonce_ref: string | null
  l402_endpoint_ref: string | null
  l402_entitlement_scope_refs_json: string | null
  l402_www_authenticate: string | null
}>

type RedemptionRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  challenge_id: string
  created_at: string
  entitlement_ref: string
  id: string
  idempotency_key: string
  proof_ref: string
  receipt_id: string | null
  replayed: number
}>

type ReceiptRow = Readonly<{
  action_kind: string
  amount_asset: 'credits' | 'sats' | 'usd'
  amount_value: number
  archived_at: string | null
  created_at: string
  id: string
  public_projection_json: string
  receipt_ref: string
  recipient_actor_ref: string | null
  redacted_payment_ref: string
  target_forum_id: string | null
  target_post_id: string | null
  target_topic_id: string | null
}>

type MoneyActionRow = Readonly<{
  action_kind: string
  amount_asset: 'credits' | 'sats' | 'usd'
  amount_value: number
  earning_actor_ref: string | null
  id: string
  payment_event_id: string | null
  public_projection_json: string
  receipt_id: string | null
}>

type PaymentEventRow = Readonly<{
  amount_asset: 'credits' | 'sats' | 'usd'
  amount_value: number
  archived_at: string | null
  created_at: string
  external_ref: string
  id: string
  money_action_id: string
  provider_ref: string
  public_projection_json: string
  redacted_evidence_ref: string
}>

type DirectTipAttemptRow = Readonly<{
  amount_sats: number
  archived_at: string | null
  created_at: string
  external_ref: string
  id: string
  idempotency_key: string
  payer_actor_ref: string
  payment_event_id: string | null
  payment_event_status:
    | 'confirmed'
    | 'failed'
    | 'observed'
    | 'refunded'
    | 'replayed'
    | 'reversed'
  payment_mode: 'live' | 'sandbox' | 'signet' | 'unknown'
  provider_ref: string
  receipt_ref: string | null
  recipient_actor_ref: string
  redacted_evidence_ref: string
  status: 'settled' | 'failed' | 'recovery_pending'
  target_post_id: string
  target_post_permalink: string | null
  target_topic_id: string
  updated_at: string
}>

type DirectTipWebhookEventRow = Readonly<{
  amount_sats: number
  archived_at: string | null
  delivery_count: number
  direct_tip_attempt_id: string
  event_body_digest_ref: string
  external_ref: string
  first_seen_at: string
  id: string
  last_seen_at: string
  payment_event_status:
    | 'confirmed'
    | 'failed'
    | 'observed'
    | 'refunded'
    | 'replayed'
    | 'reversed'
  provider_event_ref: string
  provider_ref: string
  reconciliation_result: string
  reconciliation_status: 'settled' | 'failed' | 'recovery_pending'
  redacted_evidence_ref: string
  signature_binding_ref: string
}>

type TipSettlementClaimRow = Readonly<{
  archived_at: string | null
  created_at: string
  id: string
  idempotency_key: string
  public_projection_json: string
  receipt_id: string
  receipt_ref: string
  recipient_actor_ref: string
  settlement_evidence_refs_json: string
  settlement_ref: string
  source_ref: string
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

const forumTopicPinRank = (pinState: TopicRow['pin_state']): number =>
  pinState === 'announcement' ? 0 : pinState === 'sticky' ? 1 : 2

const forumTopicActivityIso = (
  store: ForumRouteStore,
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
  store: ForumRouteStore,
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

type AgentProfileRow = Readonly<{
  avatar_url: string | null
  created_at: string
  display_name: string
  slug: string | null
  updated_at: string
  user_id: string
}>

type AgentOwnerClaimRow = Readonly<{
  agent_user_id: string
  decided_at: string | null
  id: string
  owner_user_id: string
  receipt_ref: string
  status: 'approved' | 'expired' | 'pending' | 'rejected' | 'revoked'
  updated_at: string
}>

type AgentOwnerXChallengeRow = Readonly<{
  agent_user_id: string
  id: string
  receipt_ref: string | null
  state: 'approved' | 'pending_tweet' | 'rejected' | 'verified'
  tweet_ref: string | null
  updated_at: string | null
  verified_at: string | null
}>

type WatchRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  created_at: string
  forum_id: string | null
  id: string
  idempotency_key: string
  topic_id: string | null
  watch_kind: 'forum' | 'topic'
}>

type BookmarkRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  bookmark_kind: 'topic' | 'post'
  created_at: string
  id: string
  idempotency_key: string
  post_id: string | null
  topic_id: string | null
}>

type FollowRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  created_at: string
  id: string
  idempotency_key: string
  target_actor_ref: string
}>

type ReportRow = Readonly<{
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

type PostRevisionRow = Readonly<{
  action_kind: 'edit' | 'tombstone'
  actor_ref: string
  archived_at: string | null
  created_at: string
  id: string
  idempotency_key: string
  next_body_text: string | null
  next_state: 'visible' | 'edited' | 'tombstoned' | 'held_for_review' | 'hidden'
  post_id: string
  previous_body_text: string | null
  previous_state:
    | 'visible'
    | 'edited'
    | 'tombstoned'
    | 'held_for_review'
    | 'hidden'
  public_projection_json: string
  reason_ref: string | null
}>

type ModerationEventRow = Readonly<{
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

type NotificationReadRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  created_at: string
  id: string
  idempotency_key: string
  notification_id: string
  read_at: string
  updated_at: string
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

type WorkRequestRow = Readonly<{
  archived_at: string | null
  budget_msats: number
  budget_sats: number
  created_at: string
  deadline_ref: string
  first_post_id: string
  id: string
  idempotency_key: string
  job_event_id: string
  job_event_kind: number
  job_result_kind: number
  objective_ref: string
  public_projection_json: string
  quote_count: number
  relay_url: string
  repository_refs_json: string
  requester_actor_ref: string
  required_capability_refs_json: string
  state:
    | 'open'
    | 'quote_received'
    | 'quote_accepted'
    | 'running'
    | 'delivered'
    | 'accepted'
    | 'settled'
    | 'cancelled'
    | 'expired'
  title: string
  topic_id: string
  updated_at: string
  verification_command_ref: string
}>

type WorkRequestRelayLinkRow = Readonly<{
  archived_at: string | null
  bridge_actor_ref: string
  created_at: string
  event_json: string
  id: string
  job_event_id: string
  job_event_kind: number
  relay_ref: string
  relay_url: string
  topic_id: string
  work_request_id: string
}>

type WorkRequestLifecyclePostRow = Readonly<{
  archived_at: string | null
  created_at: string
  id: string
  idempotency_key: string
  lifecycle_kind:
    | 'quote_received'
    | 'quote_accepted'
    | 'running'
    | 'delivered'
    | 'accepted'
    | 'settled'
    | 'cancelled'
    | 'expired'
  post_id: string
  receipt_ref: string
  state_after: WorkRequestRow['state']
  topic_id: string
  work_request_id: string
}>

type WorkRequestOfferRow = Readonly<{
  amount_msats: number
  amount_sats: number
  archived_at: string | null
  capability_refs_json: string
  created_at: string
  id: string
  provider_actor_ref: string
  public_projection_json: string
  quote_ref: string
  relay_event_ref: string | null
  state: 'accepted' | 'expired' | 'offered' | 'rejected'
  updated_at: string
  work_request_id: string
}>

type WorkRequestAcceptanceRow = Readonly<{
  acceptance_event_ref: string
  amount_msats: number
  archived_at: string | null
  created_at: string
  escrow_id: string
  id: string
  idempotency_key: string
  offer_id: string
  provider_actor_ref: string
  public_projection_json: string
  quote_ref: string
  requester_actor_ref: string
  reserve_receipt_ref: string
  work_request_id: string
}>

const projectionJson = JSON.stringify({
  classificationCaveatRef: 'classification.public_forum_projection',
  customerSafe: true,
  dataClassification: 'public',
  excludedPrivateRefs: [],
  publicSafe: true,
  redactionPolicyRef: 'redaction.forum.public.v1',
  safeArtifactRefs: ['artifact.forum.route_test'],
  safeReceiptRefs: [],
  trustTier: 'reviewed',
})
const privateProjectionJson = JSON.stringify({
  classificationCaveatRef: 'classification.private_forum_context',
  customerSafe: false,
  dataClassification: 'private',
  excludedPrivateRefs: ['private_context'],
  publicSafe: false,
  redactionPolicyRef: 'redaction.forum.private.v1',
  safeArtifactRefs: [],
  safeReceiptRefs: [],
  trustTier: 'restricted',
})

const actorJson = JSON.stringify({
  actorId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  actorRef: 'actor.route-test',
  displayName: 'Route Test',
  groupRefs: ['group.test'],
  isAgent: true,
  slug: 'route-test',
})
const authenticatedAgentActorJson = JSON.stringify({
  actorId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  actorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  displayName: 'Route Test Agent',
  groupRefs: ['agents'],
  isAgent: true,
  slug: 'route-test-agent',
})
const artanisActorJson = JSON.stringify({
  actorId: '99999999-9999-4999-8999-999999999999',
  actorRef: 'agent:agent_artanis',
  displayName: 'Artanis',
  groupRefs: ['agents', 'openagents'],
  isAgent: true,
  slug: 'artanis',
})

const readyTipRecipientWalletRow = (
  overrides: Partial<TipRecipientWalletRow> = {},
): TipRecipientWalletRow => ({
  actor_ref: 'actor.route-test',
  archived_at: null,
  spark_address: null,
  bolt12_offer:
    'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
  lightning_address: null,
  caveat_refs_json: JSON.stringify([
    'caveat.public.forum_tip_recipient.claim_required',
  ]),
  claim_policy_refs_json: JSON.stringify([
    'policy.public.forum_tip_recipient.agent_claimed',
  ]),
  created_at: '2026-06-06T21:00:00.000Z',
  custody_policy_refs_json: JSON.stringify([
    'policy.public.forum_tip_recipient.self_custody',
  ]),
  disabled_at: null,
  id: 'forum_tip_recipient_wallet_route_test',
  payout_target_approval_ref: 'approval.public.forum_tip_recipient.route_test',
  provider_class: 'mdk_agent_wallet',
  public_projection_json: '{}',
  readiness_refs_json: JSON.stringify([
    'readiness.public.forum_tip_recipient.receive_ready',
  ]),
  receive_capability_ref:
    'receive_capability.public.forum_tip_recipient.route_test',
  source_ref: 'source.public.pylon_api_registration.route_test',
  state: 'ready',
  updated_at: '2026-06-06T21:00:00.000Z',
  wallet_ref: 'wallet.public.forum_tip_recipient.route_test',
  ...overrides,
})

const forumHostedMdkClient = () =>
  makeFakeOpenAgentsHostedMdkClient(
    {
      configRef: 'config.forum.route.mdk.sandbox',
      credentialBindingRef: 'binding.forum.route.mdk.sandbox',
      environment: 'sandbox',
      providerRef: 'provider.forum.route.mdk.sandbox',
      webhookBindingRef: null,
    },
    { nowIso: '2026-06-05T20:00:00.000Z' },
  )

const forumL402SigningSecret = 'forum-route-test-l402-secret'
const forumMdkWebhookSecret = 'forum-route-test-mdk-webhook-secret'

const forumL402SigningBoundary = () =>
  makeOpenAgentsL402HmacSigningBoundary({
    secretKeyMaterial: forumL402SigningSecret,
    signerRef: 'binding.forum.route.mdk.sandbox',
  })

type CapturedWorkRequestRelayPublish = Readonly<{
  draft: Readonly<{
    content: string
    kind: number
    tags: ReadonlyArray<readonly string[]>
  }>
  relayUrl: string
  topicId: string
  workRequestId: string
}>

const fakeWorkRequestRelayPublisher = (
  captured: Array<CapturedWorkRequestRelayPublish>,
) => ({
  publishWorkRequest: async (input: CapturedWorkRequestRelayPublish) => {
    captured.push(input)
    const jobEventId = String(captured.length).padStart(64, '0')

    return {
      accepted: true,
      event: {
        content: input.draft.content,
        id: jobEventId,
        kind: input.draft.kind,
        pubkey: 'b'.repeat(64),
        sig: 'c'.repeat(128),
        tags: input.draft.tags,
      },
      jobEventId,
      relayRef: 'relay.public.test.openagents_market',
      relayUrl: input.relayUrl,
    }
  },
})

const signStandardWebhook = async (
  secret: string,
  id: string,
  timestamp: string,
  body: string,
): Promise<string> => {
  const encoder = new TextEncoder()
  const key = await crypto.subtle.importKey(
    'raw',
    encoder.encode(secret),
    { hash: 'SHA-256', name: 'HMAC' },
    false,
    ['sign'],
  )
  const signature = await crypto.subtle.sign(
    'HMAC',
    key,
    encoder.encode(`${id}.${timestamp}.${body}`),
  )

  return [...new Uint8Array(signature)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

class ForumRouteStore {
  failInsertsInto: string | null = null
  orangeCheckEntitlements: Array<{
    action_ref: string | null
    actor_ref: string
    agent_user_id: string
    created_at: string
    id: string
    paid_amount_cents: number
    receipt_ref: string
    state: string
    updated_at: string
  }> = []

  private idCounter = 0

  nextId(): string {
    this.idCounter += 1

    return `aaaaaaaa-1111-4111-8111-${String(this.idCounter).padStart(12, '0')}`
  }

  agentProfiles: Array<AgentProfileRow> = [
    {
      avatar_url: null,
      created_at: '2026-06-05T20:00:00.000Z',
      display_name: 'Route Test Agent',
      slug: 'route-test-agent',
      updated_at: '2026-06-05T20:00:00.000Z',
      user_id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
    },
  ]
  boards: Array<BoardRow> = [
    {
      archived_at: null,
      description_ref: 'content.forum.board.openagents.description',
      id: '11111111-1111-4111-8111-111111111111',
      public_projection_json: projectionJson,
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
    {
      archived_at: null,
      board_id: '11111111-1111-4111-8111-111111111111',
      description_ref: 'content.forum.category.agents.description',
      discoverability: 'listed',
      id: '88888888-2222-4222-8222-888888888888',
      order_index: 20,
      slug: 'agents',
      title: 'Agents',
    },
    {
      archived_at: null,
      board_id: '11111111-1111-4111-8111-111111111111',
      description_ref: 'content.forum.category.product_feedback.description',
      discoverability: 'listed',
      id: '99999999-2222-4222-8222-999999999999',
      order_index: 30,
      slug: 'product-feedback',
      title: 'Product Feedback',
    },
    {
      archived_at: null,
      board_id: '11111111-1111-4111-8111-111111111111',
      description_ref: 'content.forum.category.labor.description',
      discoverability: 'listed',
      id: '99999999-7777-4777-8777-999999999999',
      order_index: 50,
      slug: 'labor',
      title: 'Labor',
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
      latest_post_id: '66666666-6666-4666-8666-666666666666',
      latest_topic_id: '55555555-5555-4555-8555-555555555555',
      locked: 0,
      post_count: 1,
      public_projection_json: projectionJson,
      slug: 'site-builder-help',
      title: 'Site Builder Help',
      topic_count: 1,
      visibility: 'public',
    },
    {
      archived_at: null,
      board_id: '11111111-1111-4111-8111-111111111111',
      category_id: '44444444-1111-4111-8111-444444444444',
      description_ref: 'content.forum.void.description',
      discoverability: 'unlisted',
      id: '77777777-1111-4111-8111-777777777777',
      latest_post_id: null,
      latest_topic_id: null,
      locked: 0,
      post_count: 0,
      public_projection_json: projectionJson,
      slug: 'void',
      title: 'Void',
      topic_count: 0,
      visibility: 'public',
    },
    {
      archived_at: null,
      board_id: '11111111-1111-4111-8111-111111111111',
      category_id: '88888888-2222-4222-8222-888888888888',
      description_ref: 'content.forum.artanis.description',
      discoverability: 'listed',
      id: '88888888-3333-4333-8333-888888888888',
      latest_post_id: '88888888-5008-4008-8008-888888888888',
      latest_topic_id: '88888888-4008-4008-8008-888888888888',
      locked: 0,
      post_count: 8,
      public_projection_json: projectionJson,
      slug: 'artanis',
      title: 'Artanis',
      topic_count: 8,
      visibility: 'public',
    },
    {
      archived_at: null,
      board_id: '11111111-1111-4111-8111-111111111111',
      category_id: '99999999-2222-4222-8222-999999999999',
      description_ref: 'content.forum.product_promises.description',
      discoverability: 'listed',
      id: '99999999-3333-4333-8333-999999999999',
      latest_post_id: null,
      latest_topic_id: null,
      locked: 0,
      post_count: 0,
      public_projection_json: projectionJson,
      slug: 'product-promises',
      title: 'Product Promises',
      topic_count: 0,
      visibility: 'public',
    },
    {
      archived_at: null,
      board_id: '11111111-1111-4111-8111-111111111111',
      category_id: '99999999-7777-4777-8777-999999999999',
      description_ref: 'content.forum.work_requests.description',
      discoverability: 'listed',
      id: '99999999-7778-4778-8778-999999999999',
      latest_post_id: null,
      latest_topic_id: null,
      locked: 0,
      post_count: 0,
      public_projection_json: projectionJson,
      slug: 'work-requests',
      title: 'Work Requests',
      topic_count: 0,
      visibility: 'public',
    },
  ]
  topics: Array<TopicRow> = [
    {
      actor_json: actorJson,
      archived_at: null,
      created_at: '2026-06-05T20:00:00.000Z',
      first_post_id: '66666666-6666-4666-8666-666666666666',
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: '55555555-5555-4555-8555-555555555555',
      idempotency_key: 'seed-topic',
      latest_post_id: '66666666-6666-4666-8666-666666666666',
      pin_state: 'normal',
      post_count: 1,
      public_projection_json: projectionJson,
      score_ref: null,
      slug: 'first-topic',
      state: 'open',
      title: 'First Topic',
      updated_at: '2026-06-05T20:00:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      created_at: '2026-06-06T20:00:00.000Z',
      first_post_id: '88888888-5001-4001-8001-888888888888',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-4001-4001-8001-888888888888',
      idempotency_key: 'seed:artanis:status:v1',
      latest_post_id: '88888888-5001-4001-8001-888888888888',
      pin_state: 'announcement',
      post_count: 1,
      public_projection_json: projectionJson,
      score_ref: 'score.forum.artanis.status',
      slug: 'artanis-status',
      state: 'open',
      title: 'Artanis status',
      updated_at: '2026-06-06T20:00:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      created_at: '2026-06-06T20:01:00.000Z',
      first_post_id: '88888888-5002-4002-8002-888888888888',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-4002-4002-8002-888888888888',
      idempotency_key: 'seed:artanis:pylon-campaign:v1',
      latest_post_id: '88888888-5002-4002-8002-888888888888',
      pin_state: 'sticky',
      post_count: 1,
      public_projection_json: projectionJson,
      score_ref: 'score.forum.artanis.pylon_campaign',
      slug: 'pylon-campaign-status',
      state: 'open',
      title: 'Pylon campaign status',
      updated_at: '2026-06-06T20:01:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      created_at: '2026-06-06T20:02:00.000Z',
      first_post_id: '88888888-5003-4003-8003-888888888888',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-4003-4003-8003-888888888888',
      idempotency_key: 'seed:artanis:model-lab:v1',
      latest_post_id: '88888888-5003-4003-8003-888888888888',
      pin_state: 'sticky',
      post_count: 1,
      public_projection_json: projectionJson,
      score_ref: 'score.forum.artanis.model_lab',
      slug: 'model-lab',
      state: 'open',
      title: 'Model Lab',
      updated_at: '2026-06-06T20:02:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      created_at: '2026-06-06T20:03:00.000Z',
      first_post_id: '88888888-5004-4004-8004-888888888888',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-4004-4004-8004-888888888888',
      idempotency_key: 'seed:artanis:pylon-release-work-log:v1',
      latest_post_id: '88888888-5004-4004-8004-888888888888',
      pin_state: 'sticky',
      post_count: 1,
      public_projection_json: projectionJson,
      score_ref: 'score.forum.artanis.pylon_release',
      slug: 'pylon-release-work-log',
      state: 'open',
      title: 'Pylon release work log',
      updated_at: '2026-06-06T20:03:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      created_at: '2026-06-06T20:04:00.000Z',
      first_post_id: '88888888-5005-4005-8005-888888888888',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-4005-4005-8005-888888888888',
      idempotency_key: 'seed:artanis:work-routing:v1',
      latest_post_id: '88888888-5005-4005-8005-888888888888',
      pin_state: 'sticky',
      post_count: 1,
      public_projection_json: projectionJson,
      score_ref: 'score.forum.artanis.work_routing',
      slug: 'work-routing-and-accepted-outcomes',
      state: 'open',
      title: 'Work routing and accepted outcomes',
      updated_at: '2026-06-06T20:04:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      created_at: '2026-06-06T20:05:00.000Z',
      first_post_id: '88888888-5006-4006-8006-888888888888',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-4006-4006-8006-888888888888',
      idempotency_key: 'seed:artanis:bitcoin-accounting:v1',
      latest_post_id: '88888888-5006-4006-8006-888888888888',
      pin_state: 'sticky',
      post_count: 1,
      public_projection_json: projectionJson,
      score_ref: 'score.forum.artanis.bitcoin_rewards',
      slug: 'bitcoin-accounting-and-rewards',
      state: 'open',
      title: 'Bitcoin accounting and rewards',
      updated_at: '2026-06-06T20:05:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      created_at: '2026-06-06T20:06:00.000Z',
      first_post_id: '88888888-5007-4007-8007-888888888888',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-4007-4007-8007-888888888888',
      idempotency_key: 'seed:artanis:resource-modes:v1',
      latest_post_id: '88888888-5007-4007-8007-888888888888',
      pin_state: 'sticky',
      post_count: 1,
      public_projection_json: projectionJson,
      score_ref: 'score.forum.artanis.resource_modes',
      slug: 'resource-modes',
      state: 'open',
      title: 'Resource modes',
      updated_at: '2026-06-06T20:06:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      created_at: '2026-06-06T20:07:00.000Z',
      first_post_id: '88888888-5008-4008-8008-888888888888',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-4008-4008-8008-888888888888',
      idempotency_key: 'seed:artanis:operator-questions:v1',
      latest_post_id: '88888888-5008-4008-8008-888888888888',
      pin_state: 'sticky',
      post_count: 1,
      public_projection_json: projectionJson,
      score_ref: 'score.forum.artanis.operator_questions',
      slug: 'operator-questions',
      state: 'open',
      title: 'Operator questions',
      updated_at: '2026-06-06T20:07:00.000Z',
    },
  ]
  posts: Array<PostRow> = [
    {
      actor_json: actorJson,
      archived_at: null,
      body_text: 'Seed route-test body.',
      content_ref: 'content.forum.route_test.first',
      created_at: '2026-06-05T20:00:00.000Z',
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: '66666666-6666-4666-8666-666666666666',
      idempotency_key: 'seed-post',
      parent_post_id: null,
      post_number: 1,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '55555555-5555-4555-8555-555555555555',
      updated_at: '2026-06-05T20:00:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      body_text:
        'Canonical status thread for Artanis. Public updates here should summarize the active goal, loop state, approved blockers, Forum receipts, and next public checkpoint.',
      content_ref: 'content.forum.artanis.status.first',
      created_at: '2026-06-06T20:00:00.000Z',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-5001-4001-8001-888888888888',
      idempotency_key: 'seed:artanis:status:first-post:v1',
      parent_post_id: null,
      post_number: 1,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '88888888-4001-4001-8001-888888888888',
      updated_at: '2026-06-06T20:00:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      body_text:
        'Pylon campaign status thread for public Nexus and Pylon progress, launch caveats, accepted work, and proof links.',
      content_ref: 'content.forum.artanis.pylon_campaign.first',
      created_at: '2026-06-06T20:01:00.000Z',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-5002-4002-8002-888888888888',
      idempotency_key: 'seed:artanis:pylon-campaign:first-post:v1',
      parent_post_id: null,
      post_number: 1,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '88888888-4002-4002-8002-888888888888',
      updated_at: '2026-06-06T20:01:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      body_text:
        'Model Lab thread for retained failures, benchmark evidence, candidate model reports, promotion decisions, and rollback posture.',
      content_ref: 'content.forum.artanis.model_lab.first',
      created_at: '2026-06-06T20:02:00.000Z',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-5003-4003-8003-888888888888',
      idempotency_key: 'seed:artanis:model-lab:first-post:v1',
      parent_post_id: null,
      post_number: 1,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '88888888-4003-4003-8003-888888888888',
      updated_at: '2026-06-06T20:02:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      body_text:
        'Pylon release work log for v0.2 readiness, setup notes, resource-mode caveats, and launch blockers.',
      content_ref: 'content.forum.artanis.pylon_release.first',
      created_at: '2026-06-06T20:03:00.000Z',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-5004-4004-8004-888888888888',
      idempotency_key: 'seed:artanis:pylon-release:first-post:v1',
      parent_post_id: null,
      post_number: 1,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '88888888-4004-4004-8004-888888888888',
      updated_at: '2026-06-06T20:03:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      body_text:
        'Work routing and accepted outcomes thread for job intake, assignment, evidence, acceptance receipts, and public-safe closeouts.',
      content_ref: 'content.forum.artanis.work_routing.first',
      created_at: '2026-06-06T20:04:00.000Z',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-5005-4005-8005-888888888888',
      idempotency_key: 'seed:artanis:work-routing:first-post:v1',
      parent_post_id: null,
      post_number: 1,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '88888888-4005-4005-8005-888888888888',
      updated_at: '2026-06-06T20:04:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      body_text:
        'Bitcoin accounting and rewards thread for Forum participation rewards, tipping, payment receipts, and payout caveats.',
      content_ref: 'content.forum.artanis.bitcoin_rewards.first',
      created_at: '2026-06-06T20:05:00.000Z',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-5006-4006-8006-888888888888',
      idempotency_key: 'seed:artanis:bitcoin-accounting:first-post:v1',
      parent_post_id: null,
      post_number: 1,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '88888888-4006-4006-8006-888888888888',
      updated_at: '2026-06-06T20:05:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      body_text:
        'Resource modes thread for background, overnight, and dedicated Pylon compute modes, including agent-facing setup commands and safety limits.',
      content_ref: 'content.forum.artanis.resource_modes.first',
      created_at: '2026-06-06T20:06:00.000Z',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-5007-4007-8007-888888888888',
      idempotency_key: 'seed:artanis:resource-modes:first-post:v1',
      parent_post_id: null,
      post_number: 1,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '88888888-4007-4007-8007-888888888888',
      updated_at: '2026-06-06T20:06:00.000Z',
    },
    {
      actor_json: artanisActorJson,
      archived_at: null,
      body_text:
        'Operator questions thread for public-safe requests, authority boundaries, blocked decisions, and owner guidance that Artanis can answer or route.',
      content_ref: 'content.forum.artanis.operator_questions.first',
      created_at: '2026-06-06T20:07:00.000Z',
      forum_id: '88888888-3333-4333-8333-888888888888',
      id: '88888888-5008-4008-8008-888888888888',
      idempotency_key: 'seed:artanis:operator-questions:first-post:v1',
      parent_post_id: null,
      post_number: 1,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '88888888-4008-4008-8008-888888888888',
      updated_at: '2026-06-06T20:07:00.000Z',
    },
  ]
  challenges: Array<ChallengeRow> = []
  directTipAttempts: Array<DirectTipAttemptRow> = []
  directTipWebhookEvents: Array<DirectTipWebhookEventRow> = []
  redemptions: Array<RedemptionRow> = []
  receipts: Array<ReceiptRow> = []
  moneyActions: Array<MoneyActionRow> = []
  paymentEvents: Array<PaymentEventRow> = []
  tipSettlementClaims: Array<TipSettlementClaimRow> = []
  watches: Array<WatchRow> = []
  bookmarks: Array<BookmarkRow> = []
  follows: Array<FollowRow> = []
  reports: Array<ReportRow> = []
  postRevisions: Array<PostRevisionRow> = []
  moderationEvents: Array<ModerationEventRow> = []
  notificationReads: Array<NotificationReadRow> = []
  tipRecipientWallets: Array<TipRecipientWalletRow> = []
  contextLinks: Array<ContextLinkRow> = []
  workRequests: Array<WorkRequestRow> = []
  workRequestRelayLinks: Array<WorkRequestRelayLinkRow> = []
  workRequestLifecyclePosts: Array<WorkRequestLifecyclePostRow> = []
  workRequestOffers: Array<WorkRequestOfferRow> = []
  workRequestAcceptances: Array<WorkRequestAcceptanceRow> = []
  agentOwnerClaims: Array<AgentOwnerClaimRow> = []
  agentOwnerXChallenges: Array<AgentOwnerXChallengeRow> = []
}

class ForumRouteStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: ForumRouteStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM orange_check_entitlements')) {
      const actorRef = String(this.values[0])
      const entitlement = this.store.orangeCheckEntitlements.find(
        item => item.actor_ref === actorRef && item.state === 'active',
      )

      return Promise.resolve((entitlement ?? null) as T | null)
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

    if (this.query.includes('SELECT COUNT(*) AS count')) {
      const actorRef = String(this.values[0])
      let count = 0

      if (this.query.includes('FROM forum_l402_challenges')) {
        const actionKind = String(this.values[1])
        const sinceIso = String(this.values[2])
        count = this.store.challenges.filter(
          item =>
            item.actor_ref === actorRef &&
            item.action_kind === actionKind &&
            item.created_at >= sinceIso &&
            item.archived_at === null,
        ).length
      } else if (this.query.includes('FROM forum_topics')) {
        count = this.store.topics.filter(
          item =>
            item.actor_json.includes(actorRef) &&
            item.archived_at === null &&
            (item.state === 'open' || item.state === 'locked'),
        ).length
      } else if (this.query.includes('FROM forum_posts')) {
        count = this.store.posts.filter(
          item =>
            item.actor_json.includes(actorRef) &&
            item.archived_at === null &&
            (item.state === 'visible' || item.state === 'edited'),
        ).length
      } else if (this.query.includes('FROM forum_receipts')) {
        count = this.store.receipts.filter(
          item =>
            item.recipient_actor_ref === actorRef && item.archived_at === null,
        ).length
      } else if (this.query.includes('FROM forum_money_actions')) {
        const scoped = this.query.includes('ma.earning_actor_ref = ?')
        count = this.store.moneyActions.filter(
          item =>
            item.action_kind === 'post_reward' &&
            item.earning_actor_ref !== null &&
            (scoped ? item.earning_actor_ref === actorRef : true),
        ).length
      } else if (this.query.includes('FROM forum_watches')) {
        count = this.store.watches.filter(
          item => item.actor_ref === actorRef && item.archived_at === null,
        ).length
      } else if (this.query.includes('FROM forum_bookmarks')) {
        count = this.store.bookmarks.filter(
          item => item.actor_ref === actorRef && item.archived_at === null,
        ).length
      } else if (this.query.includes('FROM forum_actor_follows')) {
        count = this.store.follows.filter(
          item =>
            item.target_actor_ref === actorRef && item.archived_at === null,
        ).length
      }

      return Promise.resolve({ count } as T)
    }

    if (this.query.includes('FROM users')) {
      const userId = String(this.values[0])
      const slug = String(this.values[1])
      const row =
        this.store.agentProfiles.find(
          item => item.user_id === userId || item.slug === slug,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM agent_owner_claims')) {
      const agentUserId = String(this.values[0])
      const row =
        this.store.agentOwnerClaims
          .filter(
            item =>
              item.agent_user_id === agentUserId &&
              item.status === 'approved',
          )
          .sort(
            (left, right) =>
              (right.decided_at ?? '').localeCompare(left.decided_at ?? '') ||
              right.updated_at.localeCompare(left.updated_at),
          )[0] ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM agent_owner_x_claim_challenges')) {
      const agentUserId = String(this.values[0])
      const row =
        this.store.agentOwnerXChallenges
          .filter(
            item =>
              item.agent_user_id === agentUserId &&
              (item.state === 'verified' || item.state === 'approved') &&
              item.tweet_ref !== null,
          )
          .sort(
            (left, right) =>
              (right.verified_at ?? '').localeCompare(left.verified_at ?? '') ||
              (right.updated_at ?? '').localeCompare(left.updated_at ?? ''),
          )[0] ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_boards')) {
      const row =
        this.store.boards.find(
          item => item.slug === 'openagents' && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_watches')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.watches.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_bookmarks')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.bookmarks.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_actor_follows')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.follows.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_reports')) {
      const ref = String(this.values[0])
      const row = this.query.includes('idempotency_key =')
        ? (this.store.reports.find(
            item => item.idempotency_key === ref && item.archived_at === null,
          ) ?? null)
        : (this.store.reports.find(
            item => item.id === ref && item.archived_at === null,
          ) ?? null)

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_moderation_events')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.moderationEvents.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_post_revisions')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.postRevisions.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_notification_reads')) {
      const actorRef = String(this.values[0])
      const ref = String(this.values[1])
      const row = this.query.includes('idempotency_key =')
        ? (this.store.notificationReads.find(
            item =>
              item.actor_ref === actorRef &&
              item.idempotency_key === ref &&
              item.archived_at === null,
          ) ?? null)
        : (this.store.notificationReads.find(
            item =>
              item.actor_ref === actorRef &&
              item.notification_id === ref &&
              item.archived_at === null,
          ) ?? null)

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

    if (this.query.includes('FROM forum_tip_settlement_claims')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.tipSettlementClaims.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_l402_challenges') &&
      this.query.includes('idempotency_key = ?')
    ) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.challenges.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_direct_tip_attempts') &&
      this.query.includes('idempotency_key = ?')
    ) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.directTipAttempts.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_direct_tip_attempts') &&
      this.query.includes('provider_ref = ?') &&
      this.query.includes('external_ref = ?')
    ) {
      const providerRef = String(this.values[0])
      const externalRef = String(this.values[1])
      const row =
        this.store.directTipAttempts.find(
          item =>
            item.provider_ref === providerRef &&
            item.external_ref === externalRef &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_direct_tip_attempts') &&
      this.query.includes('id = ?')
    ) {
      const attemptId = String(this.values[0])
      const row =
        this.store.directTipAttempts.find(
          item => item.id === attemptId && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_direct_tip_webhook_events')) {
      const providerEventRef = String(this.values[0])
      const row =
        this.store.directTipWebhookEvents.find(
          item =>
            item.provider_event_ref === providerEventRef &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_money_actions') &&
      this.query.includes('payment_event_id = ?')
    ) {
      const paymentEventId = String(this.values[0])
      const row =
        this.store.moneyActions.find(
          item => item.payment_event_id === paymentEventId,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_l402_challenges') &&
      this.query.includes('id = ?')
    ) {
      const challengeId = String(this.values[0])
      const row =
        this.store.challenges.find(
          item => item.id === challengeId && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_l402_redemptions')) {
      const challengeId = String(this.values[0])
      const row =
        this.store.redemptions.find(
          item =>
            item.challenge_id === challengeId && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_receipts') &&
      this.query.includes('receipt_ref = ?')
    ) {
      const receiptRef = String(this.values[0])
      const receipt =
        this.store.receipts.find(
          item => item.receipt_ref === receiptRef && item.archived_at === null,
        ) ?? null
      const moneyAction =
        receipt === null
          ? null
          : (this.store.moneyActions.find(
              item => item.receipt_id === receipt.id,
            ) ?? null)
      const paymentEvent =
        moneyAction?.payment_event_id === null ||
        moneyAction?.payment_event_id === undefined
          ? null
          : (this.store.paymentEvents.find(
              item =>
                item.id === moneyAction.payment_event_id &&
                item.archived_at === null,
            ) ?? null)
      const settlementClaim =
        receipt === null
          ? null
          : (this.store.tipSettlementClaims.find(
              item =>
                item.receipt_id === receipt.id && item.archived_at === null,
            ) ?? null)
      const row =
        receipt === null
          ? null
          : {
              ...receipt,
              payment_event_projection_json:
                paymentEvent?.public_projection_json ?? null,
              settlement_claim_projection_json:
                settlementClaim?.public_projection_json ?? null,
            }

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_payment_events')) {
      const providerRef = String(this.values[0])
      const externalRef = String(this.values[1])
      const row =
        this.store.paymentEvents.find(
          item =>
            item.provider_ref === providerRef &&
            item.external_ref === externalRef &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM forum_receipts') &&
      this.query.includes('id = ?')
    ) {
      const receiptId = String(this.values[0])
      const row =
        this.store.receipts.find(
          item => item.id === receiptId && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_work_request_offers')) {
      const workRequestId = String(this.values[0])
      const quoteRef = String(this.values[1])
      const row =
        this.store.workRequestOffers.find(
          item =>
            item.work_request_id === workRequestId &&
            item.quote_ref === quoteRef &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_work_request_acceptances')) {
      const ref = String(this.values[0])
      const row = this.query.includes('idempotency_key =')
        ? (this.store.workRequestAcceptances.find(
            item =>
              item.idempotency_key === ref && item.archived_at === null,
          ) ?? null)
        : (this.store.workRequestAcceptances.find(
            item =>
              item.work_request_id === ref && item.archived_at === null,
          ) ?? null)

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_work_requests')) {
      const ref = String(this.values[0])
      const row = this.query.includes('idempotency_key =')
        ? (this.store.workRequests.find(
            item => item.idempotency_key === ref && item.archived_at === null,
          ) ?? null)
        : this.query.includes('job_event_id =')
          ? (this.store.workRequests.find(
              item => item.job_event_id === ref && item.archived_at === null,
            ) ?? null)
          : (this.store.workRequests.find(
              item => item.id === ref && item.archived_at === null,
            ) ?? null)

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_work_request_relay_links')) {
      const ref = String(this.values[0])
      const row = this.query.includes('job_event_id =')
        ? (this.store.workRequestRelayLinks.find(
            item => item.job_event_id === ref && item.archived_at === null,
          ) ?? null)
        : (this.store.workRequestRelayLinks.find(
            item => item.work_request_id === ref && item.archived_at === null,
          ) ?? null)

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM forum_work_request_lifecycle_posts')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.workRequestLifecyclePosts.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
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

    if (
      this.query.includes('forum_posts.actor_json AS actor_json') &&
      this.query.includes('forum_posts.actor_ref = ?')
    ) {
      const actorRef = String(this.values[0])
      const actorSlug = String(this.values[1] ?? this.values[0])
      const row =
        this.store.posts.find(
          item => {
            const actor = JSON.parse(item.actor_json) as {
              actorRef: string
              slug: string
            }

            return (
              (actor.actorRef === actorRef || actor.slug === actorSlug) &&
              item.archived_at === null &&
              (item.state === 'visible' || item.state === 'edited')
            )
          },
        ) ?? null

      return Promise.resolve(
        row === null
          ? null
          : ({
              actor_json: row.actor_json,
              created_at: row.created_at,
              updated_at: row.updated_at,
            } as T),
      )
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

    if (this.query.includes('FROM forum_topics')) {
      const topicRef = String(this.values[0])
      const row = this.query.includes('idempotency_key =')
        ? (this.store.topics.find(
            item =>
              item.idempotency_key === topicRef && item.archived_at === null,
          ) ?? null)
        : this.query.includes('id = ? OR slug = ?')
          ? (this.store.topics.find(
              item => item.id === topicRef && item.archived_at === null,
            ) ??
            this.store.topics.find(
              item => item.slug === topicRef && item.archived_at === null,
            ) ??
            null)
          : (this.store.topics.find(
              item => item.id === topicRef && item.archived_at === null,
            ) ?? null)

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('WITH RECURSIVE forum_post_ancestors')) {
      const startPostId = String(this.values[0])
      const ancestorPostId = String(this.values[1])
      const visited = new Set<string>()
      let cursor: string | null = startPostId

      while (cursor !== null && !visited.has(cursor)) {
        const row = this.store.posts.find(
          item => item.id === cursor && item.archived_at === null,
        )

        if (row === undefined) {
          break
        }

        if (row.id === ancestorPostId) {
          return Promise.resolve({ found: 1 } as T)
        }

        visited.add(row.id)
        cursor = row.parent_post_id
      }

      return Promise.resolve(null)
    }

    if (this.query.includes('FROM forum_posts')) {
      const postRef = String(this.values[0])
      const row = this.query.includes('idempotency_key =')
        ? (this.store.posts.find(
            item =>
              item.idempotency_key === postRef && item.archived_at === null,
          ) ?? null)
        : (this.store.posts.find(
            item => item.id === postRef && item.archived_at === null,
          ) ?? null)

      return Promise.resolve(row as T | null)
    }

    // The reliable-tip payments ledger (pay_ins/pay_in_legs) is empty in
    // this route fixture; ladder reads project no rows (#4753).
    if (this.query.includes('FROM pay_ins')) {
      return Promise.resolve(null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (
      this.store.failInsertsInto !== null &&
      this.query.includes(`INSERT INTO ${this.store.failInsertsInto}`)
    ) {
      return Promise.reject(
        new Error(
          `forced test failure inserting into ${this.store.failInsertsInto}`,
        ),
      )
    }

    if (this.query.includes('INSERT INTO forum_work_requests')) {
      this.store.workRequests.push({
        archived_at: null,
        budget_msats: Number(this.values[11]),
        budget_sats: Number(this.values[10]),
        created_at: String(this.values[18]),
        deadline_ref: String(this.values[12]),
        first_post_id: String(this.values[3]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        job_event_id: String(this.values[14]),
        job_event_kind: Number(this.values[15]),
        job_result_kind: Number(this.values[16]),
        objective_ref: String(this.values[6]),
        public_projection_json: String(this.values[17]),
        quote_count: 0,
        relay_url: String(this.values[13]),
        repository_refs_json: String(this.values[8]),
        requester_actor_ref: String(this.values[4]),
        required_capability_refs_json: String(this.values[9]),
        state: 'open',
        title: String(this.values[5]),
        topic_id: String(this.values[2]),
        updated_at: String(this.values[19]),
        verification_command_ref: String(this.values[7]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_work_request_relay_links')) {
      this.store.workRequestRelayLinks.push({
        archived_at: null,
        bridge_actor_ref: String(this.values[7]),
        created_at: String(this.values[9]),
        event_json: String(this.values[8]),
        id: String(this.values[0]),
        job_event_id: String(this.values[3]),
        job_event_kind: Number(this.values[4]),
        relay_ref: String(this.values[6]),
        relay_url: String(this.values[5]),
        topic_id: String(this.values[2]),
        work_request_id: String(this.values[1]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_work_request_lifecycle_posts')) {
      this.store.workRequestLifecyclePosts.push({
        archived_at: null,
        created_at: String(this.values[8]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[4]),
        lifecycle_kind:
          this.values[5] as WorkRequestLifecyclePostRow['lifecycle_kind'],
        post_id: String(this.values[3]),
        receipt_ref: String(this.values[6]),
        state_after: this.values[7] as WorkRequestRow['state'],
        topic_id: String(this.values[2]),
        work_request_id: String(this.values[1]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_work_request_offers')) {
      this.store.workRequestOffers.push({
        amount_msats: Number(this.values[5]),
        amount_sats: Number(this.values[4]),
        archived_at: null,
        capability_refs_json: String(this.values[6]),
        created_at: String(this.values[9]),
        id: String(this.values[0]),
        provider_actor_ref: String(this.values[3]),
        public_projection_json: String(this.values[8]),
        quote_ref: String(this.values[2]),
        relay_event_ref:
          this.values[7] === null ? null : String(this.values[7]),
        state: 'offered',
        updated_at: String(this.values[10]),
        work_request_id: String(this.values[1]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_work_request_acceptances')) {
      this.store.workRequestAcceptances.push({
        acceptance_event_ref: String(this.values[10]),
        amount_msats: Number(this.values[7]),
        archived_at: null,
        created_at: String(this.values[12]),
        escrow_id: String(this.values[8]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        offer_id: String(this.values[3]),
        provider_actor_ref: String(this.values[6]),
        public_projection_json: String(this.values[11]),
        quote_ref: String(this.values[4]),
        requester_actor_ref: String(this.values[5]),
        reserve_receipt_ref: String(this.values[9]),
        work_request_id: String(this.values[2]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_work_request_offers')) {
      const quoteRef = String(this.values[0])
      const updatedAt = String(this.values[1])
      const workRequestId = String(this.values[2])

      this.store.workRequestOffers = this.store.workRequestOffers.map(offer =>
        offer.work_request_id === workRequestId &&
        offer.state === 'offered' &&
        offer.archived_at === null
          ? {
              ...offer,
              state: offer.quote_ref === quoteRef ? 'accepted' : 'rejected',
              updated_at: updatedAt,
            }
          : offer,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_work_requests')) {
      const quoteAccepted = this.query.includes("state = 'quote_accepted'")
      const state = quoteAccepted
        ? 'quote_accepted'
        : (this.values[0] as WorkRequestRow['state'])
      const lifecycleKind = quoteAccepted ? 'quote_accepted' : String(this.values[1])
      const updatedAt = String(quoteAccepted ? this.values[0] : this.values[2])
      const workRequestId = String(quoteAccepted ? this.values[1] : this.values[3])
      const existingIndex = this.store.workRequests.findIndex(
        item => item.id === workRequestId && item.archived_at === null,
      )

      if (existingIndex !== -1) {
        const existing = this.store.workRequests[existingIndex]!
        this.store.workRequests[existingIndex] = {
          ...existing,
          quote_count:
            lifecycleKind === 'quote_received'
              ? existing.quote_count + 1
              : existing.quote_count,
          state,
          updated_at: updatedAt,
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO forum_l402_challenges')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.challenges.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.challenges.push({
          action_kind: String(this.values[3]),
          actor_ref: String(this.values[2]),
          archived_at: null,
          created_at: String(this.values[33]),
          expires_at: String(this.values[17]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          method: this.values[4] as 'POST',
          path: String(this.values[5]),
          price_asset: this.values[13] as 'credits' | 'sats' | 'usd',
          price_value: Number(this.values[14]),
          public_projection_json: String(this.values[32]),
          recipient_actor_ref:
            this.values[11] === null ? null : String(this.values[11]),
          recipient_readiness_ref:
            this.values[12] === null ? null : String(this.values[12]),
          request_body_digest: String(this.values[7]),
          route_params_json: String(this.values[6]),
          spend_cap_asset: this.values[15] as 'credits' | 'sats' | 'usd',
          spend_cap_value: Number(this.values[16]),
          target_forum_id:
            this.values[8] === null ? null : String(this.values[8]),
          target_post_id:
            this.values[10] === null ? null : String(this.values[10]),
          target_topic_id:
            this.values[9] === null ? null : String(this.values[9]),
          mdk_provider_ref:
            this.values[18] === null ? null : String(this.values[18]),
          mdk_environment:
            this.values[19] === null
              ? null
              : (this.values[19] as 'production' | 'sandbox'),
          mdk_sandbox:
            this.values[20] === null ? null : Number(this.values[20]),
          mdk_implementation_state:
            this.values[21] === null
              ? null
              : (this.values[21] as ChallengeRow['mdk_implementation_state']),
          mdk_checkout_ref:
            this.values[22] === null ? null : String(this.values[22]),
          mdk_checkout_url_ref:
            this.values[23] === null ? null : String(this.values[23]),
          mdk_checkout_launch_path:
            this.values[24] === null ? null : String(this.values[24]),
          mdk_invoice_ref:
            this.values[25] === null ? null : String(this.values[25]),
          mdk_payment_hash_ref:
            this.values[26] === null ? null : String(this.values[26]),
          l402_credential_ref:
            this.values[27] === null ? null : String(this.values[27]),
          l402_replay_nonce_ref:
            this.values[28] === null ? null : String(this.values[28]),
          l402_endpoint_ref:
            this.values[29] === null ? null : String(this.values[29]),
          l402_entitlement_scope_refs_json:
            this.values[30] === null ? null : String(this.values[30]),
          l402_www_authenticate:
            this.values[31] === null ? null : String(this.values[31]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

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
        provider_class: this
          .values[2] as TipRecipientWalletRow['provider_class'],
        public_projection_json: String(this.values[15]),
        readiness_refs_json: String(this.values[9]),
        receive_capability_ref: String(this.values[4]),
        source_ref: String(this.values[13]),
        state: this.values[14] as TipRecipientWalletRow['state'],
        updated_at: String(this.values[17]),
        wallet_ref: String(this.values[3]),
      }
      const existingIndex = this.store.tipRecipientWallets.findIndex(
        item => item.actor_ref === actorRef,
      )

      if (existingIndex === -1) {
        this.store.tipRecipientWallets.push(row)
      } else {
        const existing = this.store.tipRecipientWallets[existingIndex]!

        this.store.tipRecipientWallets[existingIndex] = {
          ...row,
          created_at: existing.created_at,
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('INSERT INTO forum_receipts') &&
      this.query.includes("VALUES (?, ?, 'post_reward', NULL")
    ) {
      this.store.receipts.push({
        action_kind: 'post_reward',
        amount_asset: 'sats',
        amount_value: Number(this.values[4]),
        archived_at: null,
        created_at: String(this.values[8]),
        id: String(this.values[0]),
        public_projection_json: String(this.values[7]),
        receipt_ref: String(this.values[1]),
        recipient_actor_ref: String(this.values[5]),
        redacted_payment_ref: String(this.values[6]),
        target_forum_id: null,
        target_post_id: String(this.values[3]),
        target_topic_id: String(this.values[2]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_receipts')) {
      this.store.receipts.push({
        action_kind: String(this.values[2]),
        amount_asset: this.values[6] as 'credits' | 'sats' | 'usd',
        amount_value: Number(this.values[7]),
        archived_at: null,
        created_at: String(this.values[11]),
        id: String(this.values[0]),
        public_projection_json: String(this.values[10]),
        receipt_ref: String(this.values[1]),
        recipient_actor_ref:
          this.values[8] === null ? null : String(this.values[8]),
        redacted_payment_ref: String(this.values[9]),
        target_forum_id:
          this.values[3] === null ? null : String(this.values[3]),
        target_post_id: this.values[5] === null ? null : String(this.values[5]),
        target_topic_id:
          this.values[4] === null ? null : String(this.values[4]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_direct_tip_attempts')) {
      this.store.directTipAttempts.push({
        amount_sats: Number(this.values[7]),
        archived_at: null,
        created_at: String(this.values[16]),
        external_ref: String(this.values[9]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        payer_actor_ref: String(this.values[2]),
        payment_event_id:
          this.values[15] === null ? null : String(this.values[15]),
        payment_event_status:
          this.values[12] as DirectTipAttemptRow['payment_event_status'],
        payment_mode: this.values[11] as DirectTipAttemptRow['payment_mode'],
        provider_ref: String(this.values[8]),
        receipt_ref: this.values[14] === null ? null : String(this.values[14]),
        recipient_actor_ref: String(this.values[3]),
        redacted_evidence_ref: String(this.values[10]),
        status: this.values[13] as DirectTipAttemptRow['status'],
        target_post_id: String(this.values[5]),
        target_post_permalink:
          this.values[6] === null ? null : String(this.values[6]),
        target_topic_id: String(this.values[4]),
        updated_at: String(this.values[17]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('INSERT OR IGNORE INTO forum_money_actions') &&
      this.query.includes("VALUES (?, ?, ?, 'post_reward', NULL")
    ) {
      this.store.moneyActions.push({
        action_kind: 'post_reward',
        amount_asset: 'sats',
        amount_value: Number(this.values[5]),
        earning_actor_ref: String(this.values[8]),
        id: String(this.values[0]),
        payment_event_id:
          this.values[6] === null ? null : String(this.values[6]),
        public_projection_json: String(this.values[9]),
        receipt_id: this.values[7] === null ? null : String(this.values[7]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO forum_money_actions')) {
      this.store.moneyActions.push({
        action_kind: String(this.values[3]),
        amount_asset: this.values[7] as 'credits' | 'sats' | 'usd',
        amount_value: Number(this.values[8]),
        earning_actor_ref:
          this.values[11] === null ? null : String(this.values[11]),
        id: String(this.values[0]),
        payment_event_id:
          this.values[9] === null ? null : String(this.values[9]),
        public_projection_json: String(this.values[12]),
        receipt_id: this.values[10] === null ? null : String(this.values[10]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('INSERT INTO forum_payment_events') &&
      this.query.includes("VALUES (?, ?, ?, ?, 'sats'")
    ) {
      this.store.paymentEvents.push({
        amount_asset: 'sats',
        amount_value: Number(this.values[4]),
        archived_at: null,
        created_at: String(this.values[7]),
        external_ref: String(this.values[3]),
        id: String(this.values[0]),
        money_action_id: String(this.values[1]),
        provider_ref: String(this.values[2]),
        public_projection_json: String(this.values[6]),
        redacted_evidence_ref: String(this.values[5]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_payment_events')) {
      this.store.paymentEvents.push({
        amount_asset: this.values[4] as 'credits' | 'sats' | 'usd',
        amount_value: Number(this.values[5]),
        archived_at: null,
        created_at: String(this.values[8]),
        external_ref: String(this.values[3]),
        id: String(this.values[0]),
        money_action_id: String(this.values[1]),
        provider_ref: String(this.values[2]),
        public_projection_json: String(this.values[7]),
        redacted_evidence_ref: String(this.values[6]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('INSERT OR IGNORE INTO forum_tip_settlement_claims')
    ) {
      const idempotencyKey = String(this.values[1])
      const receiptId = String(this.values[2])
      const exists = this.store.tipSettlementClaims.some(
        item =>
          (item.idempotency_key === idempotencyKey ||
            item.receipt_id === receiptId) &&
          item.archived_at === null,
      )

      if (!exists) {
        this.store.tipSettlementClaims.push({
          archived_at: null,
          created_at: String(this.values[9]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          public_projection_json: String(this.values[8]),
          receipt_id: receiptId,
          receipt_ref: String(this.values[3]),
          recipient_actor_ref: String(this.values[4]),
          settlement_evidence_refs_json: String(this.values[6]),
          settlement_ref: String(this.values[5]),
          source_ref: String(this.values[7]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_l402_redemptions')) {
      this.store.redemptions.push({
        actor_ref: String(this.values[3]),
        archived_at: null,
        challenge_id: String(this.values[2]),
        created_at: String(this.values[8]),
        entitlement_ref: String(this.values[5]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        proof_ref: String(this.values[4]),
        receipt_id: this.values[6] === null ? null : String(this.values[6]),
        replayed: 0,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_direct_tip_webhook_events')) {
      this.store.directTipWebhookEvents.push({
        amount_sats: Number(this.values[5]),
        archived_at: null,
        delivery_count: 1,
        direct_tip_attempt_id: String(this.values[2]),
        event_body_digest_ref: String(this.values[8]),
        external_ref: String(this.values[4]),
        first_seen_at: String(this.values[12]),
        id: String(this.values[0]),
        last_seen_at: String(this.values[13]),
        payment_event_status:
          this.values[6] as DirectTipWebhookEventRow['payment_event_status'],
        provider_event_ref: String(this.values[1]),
        provider_ref: String(this.values[3]),
        reconciliation_result: String(this.values[11]),
        reconciliation_status:
          this.values[10] as DirectTipWebhookEventRow['reconciliation_status'],
        redacted_evidence_ref: String(this.values[7]),
        signature_binding_ref: String(this.values[9]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_direct_tip_webhook_events')) {
      const providerEventRef = String(this.values[1])
      const row = this.store.directTipWebhookEvents.find(
        item => item.provider_event_ref === providerEventRef,
      )

      if (row !== undefined) {
        const index = this.store.directTipWebhookEvents.indexOf(row)
        this.store.directTipWebhookEvents[index] = {
          ...row,
          delivery_count: row.delivery_count + 1,
          last_seen_at: String(this.values[0]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_money_actions')) {
      const receiptId = String(this.values[0])
      const moneyActionId = String(this.values[1])
      const row = this.store.moneyActions.find(item => item.id === moneyActionId)

      if (row !== undefined) {
        const index = this.store.moneyActions.indexOf(row)
        this.store.moneyActions[index] = { ...row, receipt_id: receiptId }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_payment_events')) {
      const paymentEventId = String(this.values[4])
      const row = this.store.paymentEvents.find(
        item => item.id === paymentEventId,
      )

      if (row !== undefined) {
        const index = this.store.paymentEvents.indexOf(row)
        this.store.paymentEvents[index] = {
          ...row,
          external_ref: String(this.values[1]),
          provider_ref: String(this.values[0]),
          public_projection_json: String(this.values[3]),
          redacted_evidence_ref: String(this.values[2]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_direct_tip_attempts')) {
      const attemptId = String(this.values[8])
      const row = this.store.directTipAttempts.find(
        item => item.id === attemptId,
      )

      if (row !== undefined) {
        const index = this.store.directTipAttempts.indexOf(row)
        this.store.directTipAttempts[index] = {
          ...row,
          external_ref: String(this.values[1]),
          payment_event_status:
            this.values[4] as DirectTipAttemptRow['payment_event_status'],
          payment_mode: this.values[3] as DirectTipAttemptRow['payment_mode'],
          provider_ref: String(this.values[0]),
          receipt_ref:
            row.receipt_ref ??
            (this.values[6] === null ? null : String(this.values[6])),
          redacted_evidence_ref: String(this.values[2]),
          status: this.values[5] as DirectTipAttemptRow['status'],
          updated_at: String(this.values[7]),
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
          archived_at: null,
          created_at: String(this.values[6]),
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
          archived_at: null,
          bookmark_kind: this.values[4] as 'topic' | 'post',
          created_at: String(this.values[6]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          post_id: this.values[3] === null ? null : String(this.values[3]),
          topic_id: this.values[2] === null ? null : String(this.values[2]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO forum_actor_follows')) {
      const idempotencyKey = String(this.values[3])

      if (
        this.store.follows.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.follows.push({
          actor_ref: String(this.values[1]),
          archived_at: null,
          created_at: String(this.values[4]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          target_actor_ref: String(this.values[2]),
        })
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
          archived_at: null,
          body_text: null,
          content_ref: String(this.values[6]),
          created_at: String(firstPost ? this.values[8] : this.values[11]),
          forum_id: String(this.values[3]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          parent_post_id:
            firstPost || this.values[7] === null
              ? null
              : String(this.values[7]),
          post_number: firstPost ? 1 : Number(this.values[9]),
          public_projection_json: String(
            firstPost ? this.values[7] : this.values[10],
          ),
          quote_post_id:
            firstPost || this.values[8] === null
              ? null
              : String(this.values[8]),
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

    if (this.query.includes('INSERT INTO forum_reports')) {
      this.store.reports.push({
        archived_at: null,
        created_at: String(this.values[7]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        public_projection_json: String(this.values[6]),
        reason_ref: String(this.values[5]),
        reporter_actor_ref: String(this.values[2]),
        status: 'open',
        target_id: String(this.values[4]),
        target_kind: this.values[3] as 'forum' | 'topic' | 'post' | 'user',
        updated_at: String(this.values[8]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_moderation_events')) {
      this.store.moderationEvents.push({
        action_kind: String(this.values[3]),
        archived_at: null,
        created_at: String(this.values[9]),
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

    if (this.query.includes('INSERT OR IGNORE INTO forum_notification_reads')) {
      const actorRef = String(this.values[1])
      const notificationId = String(this.values[2])

      if (
        this.store.notificationReads.every(
          item =>
            item.actor_ref !== actorRef ||
            item.notification_id !== notificationId,
        )
      ) {
        this.store.notificationReads.push({
          actor_ref: actorRef,
          archived_at: null,
          created_at: String(this.values[5]),
          id: String(this.values[0]),
          idempotency_key: String(this.values[3]),
          notification_id: notificationId,
          read_at: String(this.values[4]),
          updated_at: String(this.values[6]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO forum_post_revisions')) {
      this.store.postRevisions.push({
        action_kind: this.values[4] as 'edit' | 'tombstone',
        actor_ref: String(this.values[3]),
        archived_at: null,
        created_at: String(this.values[11]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        next_body_text: this.values[6] === null ? null : String(this.values[6]),
        next_state: this.values[8] as
          | 'visible'
          | 'edited'
          | 'tombstoned'
          | 'held_for_review'
          | 'hidden',
        post_id: String(this.values[2]),
        previous_body_text:
          this.values[5] === null ? null : String(this.values[5]),
        previous_state: this.values[7] as
          | 'visible'
          | 'edited'
          | 'tombstoned'
          | 'held_for_review'
          | 'hidden',
        public_projection_json: String(this.values[10]),
        reason_ref: this.values[9] === null ? null : String(this.values[9]),
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

    if (this.query.includes('UPDATE forum_post_bodies')) {
      const postId = String(this.values[2])
      const existing = this.store.posts.find(item => item.id === postId)

      if (existing !== undefined) {
        const index = this.store.posts.findIndex(item => item.id === postId)
        const tombstone = this.query.includes('archived_at = ?')

        this.store.posts[index] = {
          ...existing,
          body_text: tombstone ? null : String(this.values[0]),
          updated_at: String(this.values[1]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE forum_posts') &&
      this.query.includes('SET parent_post_id = ?')
    ) {
      const postId = String(this.values[2])
      const existing = this.store.posts.find(item => item.id === postId)

      if (existing !== undefined) {
        const index = this.store.posts.findIndex(item => item.id === postId)

        this.store.posts[index] = {
          ...existing,
          parent_post_id:
            this.values[0] === null ? null : String(this.values[0]),
          updated_at: String(this.values[1]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_posts')) {
      const postId = String(this.values[2])
      const existing = this.store.posts.find(item => item.id === postId)

      if (existing !== undefined) {
        const index = this.store.posts.findIndex(item => item.id === postId)

        this.store.posts[index] = {
          ...existing,
          revision_ref: this.query.includes('revision_ref')
            ? String(this.values[0])
            : existing.revision_ref,
          state: this.query.includes('revision_ref')
            ? this.query.includes("'tombstoned'")
              ? 'tombstoned'
              : 'edited'
            : (this.values[0] as
                | 'visible'
                | 'edited'
                | 'tombstoned'
                | 'held_for_review'
                | 'hidden'),
          updated_at: String(this.values[1]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_forums')) {
      const isDecrement = this.query.includes(
        'post_count = MAX(0, post_count - 1)',
      )
      // The decrement (post tombstone) binds only [forumId]; the increment
      // binds [latestTopicId, latestPostId, ..., forumId] with forumId last.
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

    if (this.query.includes('UPDATE forum_reports')) {
      const reportId = String(this.values[2])
      const existing = this.store.reports.find(item => item.id === reportId)

      if (existing !== undefined) {
        const index = this.store.reports.findIndex(item => item.id === reportId)

        this.store.reports[index] = {
          ...existing,
          status: this.values[0] as
            | 'open'
            | 'reviewing'
            | 'resolved'
            | 'dismissed',
          updated_at: String(this.values[1]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO orange_check_entitlements')) {
      const actorRef = String(this.values[2])
      const existing = this.store.orangeCheckEntitlements.find(
        item => item.actor_ref === actorRef,
      )

      if (existing === undefined) {
        this.store.orangeCheckEntitlements.push({
          action_ref: String(this.values[4]),
          actor_ref: actorRef,
          agent_user_id: String(this.values[1]),
          created_at: String(this.values[6]),
          id: String(this.values[0]),
          paid_amount_cents: Number(this.values[5]),
          receipt_ref: String(this.values[3]),
          state: 'active',
          updated_at: String(this.values[7]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE forum_topics')) {
      const topicId = String(this.values[this.values.length - 1])
      const existing = this.store.topics.find(item => item.id === topicId)

      if (existing !== undefined) {
        const index = this.store.topics.findIndex(item => item.id === topicId)

        this.store.topics[index] = {
          ...existing,
          latest_post_id: this.query.includes('latest_post_id')
            ? String(this.values[0])
            : existing.latest_post_id,
          pin_state: this.query.includes('SET pin_state')
            ? (this.values[0] as 'normal' | 'sticky' | 'announcement')
            : existing.pin_state,
          post_count: this.query.includes('post_count = post_count + 1')
            ? existing.post_count + 1
            : this.query.includes('post_count = MAX(0, post_count - 1)')
              ? Math.max(0, existing.post_count - 1)
              : existing.post_count,
          state: this.query.includes('latest_post_id') ||
            this.query.includes('SET pin_state') ||
            this.query.includes('SET title') ||
            this.query.includes('post_count = MAX(0, post_count - 1)')
            ? existing.state
            : (this.values[0] as 'open' | 'locked' | 'archived' | 'hidden'),
          title: this.query.includes('SET title')
            ? String(this.values[0])
            : existing.title,
          // The post-tombstone decrement binds [now, topicId]; its timestamp is
          // values[0]. Other updates bind the timestamp at values[1].
          updated_at: this.query.includes('post_count = MAX(0, post_count - 1)')
            ? String(this.values[0])
            : String(this.values[1]),
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM forum_work_request_offers')) {
      const workRequestId = String(this.values[0])
      const rows = this.store.workRequestOffers
        .filter(
          item =>
            item.work_request_id === workRequestId &&
            item.archived_at === null,
        )
        .sort(
          (left, right) =>
            right.created_at.localeCompare(left.created_at) ||
            right.id.localeCompare(left.id),
        )

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM forum_work_requests')) {
      const limit = Number(this.values[0] ?? 50)
      const rows = this.store.workRequests
        .filter(
          item =>
            item.archived_at === null &&
            ['open', 'quote_received', 'quote_accepted', 'running'].includes(
              item.state,
            ),
        )
        .sort(
          (left, right) =>
            right.created_at.localeCompare(left.created_at) ||
            right.id.localeCompare(left.id),
        )
        .slice(0, limit)

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    if (
      this.query.includes('FROM forum_watches') &&
      this.query.includes('JOIN forum_posts')
    ) {
      const actorRef = String(this.values[0])
      const rows = this.store.posts.filter(item => {
        const actor = JSON.parse(item.actor_json) as { actorRef: string }

        return (
          this.store.watches.some(
            watch =>
              watch.actor_ref === actorRef &&
              watch.watch_kind === 'topic' &&
              watch.topic_id === item.topic_id &&
              watch.archived_at === null,
          ) &&
          actor.actorRef !== actorRef &&
          item.archived_at === null &&
          (item.state === 'visible' || item.state === 'edited')
        )
      })

      return Promise.resolve({
        results: rows.map(row => ({
          ...row,
          topic_slug:
            this.store.topics.find(topic => topic.id === row.topic_id)?.slug ??
            '',
          topic_title:
            this.store.topics.find(topic => topic.id === row.topic_id)?.title ??
            '',
        })),
      } as unknown as D1Result<T>)
    }

    if (
      this.query.includes('FROM forum_watches') &&
      this.query.includes('JOIN forum_topics')
    ) {
      const actorRef = String(this.values[0])
      const rows = this.store.topics.filter(item => {
        const actor = JSON.parse(item.actor_json) as { actorRef: string }

        return (
          this.store.watches.some(
            watch =>
              watch.actor_ref === actorRef &&
              watch.watch_kind === 'forum' &&
              watch.forum_id === item.forum_id &&
              watch.archived_at === null,
          ) &&
          actor.actorRef !== actorRef &&
          item.archived_at === null &&
          (item.state === 'open' || item.state === 'locked')
        )
      })

      return Promise.resolve({
        results: rows.map(row => ({
          ...row,
          forum_slug:
            this.store.forums.find(forum => forum.id === row.forum_id)?.slug ??
            '',
        })),
      } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM forum_actor_follows')) {
      const actorRef = String(this.values[0])
      const followedRefs = this.store.follows
        .filter(
          item => item.actor_ref === actorRef && item.archived_at === null,
        )
        .map(item => item.target_actor_ref)
      const rows = this.store.posts.filter(item => {
        const actor = JSON.parse(item.actor_json) as { actorRef: string }

        return (
          followedRefs.includes(actor.actorRef) &&
          item.archived_at === null &&
          (item.state === 'visible' || item.state === 'edited')
        )
      })

      return Promise.resolve({
        results: rows.map(row => ({
          ...row,
          topic_slug:
            this.store.topics.find(topic => topic.id === row.topic_id)?.slug ??
            '',
          topic_title:
            this.store.topics.find(topic => topic.id === row.topic_id)?.title ??
            '',
        })),
      } as unknown as D1Result<T>)
    }

    if (
      this.query.includes('FROM forum_posts') &&
      this.query.includes('forum_posts.actor_ref <> ?') &&
      this.query.includes('body_text LIKE ?')
    ) {
      const actorRef = String(this.values[0])
      const needle = String(this.values[1]).replaceAll('%', '').toLowerCase()
      const rows = this.store.posts.filter(item => {
        const actor = JSON.parse(item.actor_json) as { actorRef: string }

        return (
          actor.actorRef !== actorRef &&
          (item.body_text ?? '').toLowerCase().includes(needle) &&
          item.archived_at === null &&
          (item.state === 'visible' || item.state === 'edited')
        )
      })

      return Promise.resolve({
        results: rows.map(row => ({
          ...row,
          topic_slug:
            this.store.topics.find(topic => topic.id === row.topic_id)?.slug ??
            '',
          topic_title:
            this.store.topics.find(topic => topic.id === row.topic_id)?.title ??
            '',
        })),
      } as unknown as D1Result<T>)
    }

    if (
      this.query.includes('FROM forum_money_actions') &&
      this.query.includes('JOIN forum_receipts')
    ) {
      const paymentProjectionForAction = (action: MoneyActionRow) => {
        const paymentEvent =
          action.payment_event_id === null
            ? null
            : this.store.paymentEvents.find(
                item =>
                  item.id === action.payment_event_id &&
                  item.archived_at === null,
              )

        if (paymentEvent === null || paymentEvent === undefined) {
          return null
        }

        return JSON.parse(paymentEvent.public_projection_json) as {
          settlementAuthority?: string
          status?: string
        }
      }
      const paymentStatusForAction = (action: MoneyActionRow) =>
        paymentProjectionForAction(action)?.status
      const paymentIsRecipientSettledForAction = (action: MoneyActionRow) =>
        paymentProjectionForAction(action)?.settlementAuthority ===
        'recipient_wallet_direct'
      if (this.query.includes('ma.target_post_id AS post_id')) {
        const scopedPostIds = this.query.includes('ma.target_post_id IN (')
          ? new Set(this.values.map(String))
          : null
        const limit = scopedPostIds === null ? Number(this.values[0]) : 100
        const grouped = new Map<
          string,
          {
            actor_json: string
            post_id: string
            tip_count: number
            topic_id: string
            total_paid_sats: number
            total_settled_sats: number
          }
        >()

        for (const action of this.store.moneyActions) {
          const receipt = this.store.receipts.find(
            item => item.id === action.receipt_id && item.archived_at === null,
          )

          if (
            action.action_kind !== 'post_reward' ||
            action.amount_asset !== 'sats' ||
            receipt?.target_post_id === null ||
            receipt?.target_post_id === undefined ||
            receipt.target_topic_id === null ||
            (scopedPostIds !== null &&
              !scopedPostIds.has(receipt.target_post_id))
          ) {
            continue
          }

          const status = paymentStatusForAction(action)

          if (status !== 'confirmed') {
            continue
          }

          const post = this.store.posts.find(
            item => item.id === receipt.target_post_id,
          )

          if (post === undefined) {
            continue
          }

          const current = grouped.get(receipt.target_post_id) ?? {
            actor_json: post.actor_json,
            post_id: receipt.target_post_id,
            tip_count: 0,
            topic_id: receipt.target_topic_id,
            total_paid_sats: 0,
            total_settled_sats: 0,
          }
          current.tip_count += 1
          current.total_paid_sats += action.amount_value
          if (paymentIsRecipientSettledForAction(action)) {
            current.total_settled_sats += action.amount_value
          }
          grouped.set(receipt.target_post_id, current)
        }

        const rows = [...grouped.values()]
          .filter(row =>
            this.query.includes('HAVING total_settled_sats > 0')
              ? row.total_settled_sats > 0
              : true,
          )
          .sort(
            (left, right) =>
              right.total_paid_sats - left.total_paid_sats ||
              right.tip_count - left.tip_count ||
              left.post_id.localeCompare(right.post_id),
          )
          .slice(0, limit)

        return Promise.resolve({ results: rows } as unknown as D1Result<T>)
      }

      if (
        this.query.includes('p.actor_json AS actor_json') &&
        this.query.includes('GROUP BY ma.earning_actor_ref')
      ) {
        const limit = Number(this.values[0])
        const grouped = new Map<
          string,
          {
            actor_json: string
            tip_count: number
            total_paid_sats: number
            total_settled_sats: number
          }
        >()

        for (const action of this.store.moneyActions) {
          const receipt = this.store.receipts.find(
            item => item.id === action.receipt_id && item.archived_at === null,
          )

          if (
            action.action_kind !== 'post_reward' ||
            action.amount_asset !== 'sats' ||
            action.earning_actor_ref === null ||
            receipt?.target_post_id === null ||
            receipt?.target_post_id === undefined
          ) {
            continue
          }

          const status = paymentStatusForAction(action)

          if (status !== 'confirmed') {
            continue
          }

          const post = this.store.posts.find(
            item => item.id === receipt.target_post_id,
          )

          if (post === undefined) {
            continue
          }

          const current = grouped.get(action.earning_actor_ref) ?? {
            actor_json: post.actor_json,
            tip_count: 0,
            total_paid_sats: 0,
            total_settled_sats: 0,
          }
          current.tip_count += 1
          current.total_paid_sats += action.amount_value
          if (paymentIsRecipientSettledForAction(action)) {
            current.total_settled_sats += action.amount_value
          }
          grouped.set(action.earning_actor_ref, current)
        }

        const rows = [...grouped.values()]
          .filter(row =>
            this.query.includes('HAVING total_settled_sats > 0')
              ? row.total_settled_sats > 0
              : true,
          )
          .sort(
            (left, right) =>
              right.total_paid_sats - left.total_paid_sats ||
              right.tip_count - left.tip_count,
          )
          .slice(0, limit)

        return Promise.resolve({ results: rows } as unknown as D1Result<T>)
      }

      const scoped = this.query.includes('ma.earning_actor_ref = ?')
      const actorRef = scoped ? String(this.values[0]) : null
      const limit = Number(scoped ? this.values[1] : this.values[0])
      const rows = this.store.moneyActions
        .filter(
          action =>
            action.action_kind === 'post_reward' &&
            action.earning_actor_ref !== null &&
            (actorRef === null ? true : action.earning_actor_ref === actorRef),
        )
        .slice(0, limit)
        .flatMap(action => {
          const receipt = this.store.receipts.find(
            item => item.id === action.receipt_id && item.archived_at === null,
          )

          if (receipt === undefined) {
            return []
          }

          const paymentEvent =
            action.payment_event_id === null
              ? null
              : (this.store.paymentEvents.find(
                  item =>
                    item.id === action.payment_event_id &&
                    item.archived_at === null,
                ) ?? null)
          const settlementClaim = this.store.tipSettlementClaims.find(
            item => item.receipt_id === receipt.id && item.archived_at === null,
          )

          return [
            {
              action_kind: action.action_kind,
              amount_asset: action.amount_asset,
              amount_value: action.amount_value,
              earning_actor_ref: action.earning_actor_ref,
              money_action_created_at:
                paymentEvent?.created_at ?? receipt.created_at,
              money_action_id: action.id,
              payment_event_id: action.payment_event_id,
              payment_event_projection_json:
                paymentEvent?.public_projection_json ?? null,
              settlement_claim_projection_json:
                settlementClaim?.public_projection_json ?? null,
              receipt_ref: receipt.receipt_ref,
              recipient_actor_ref: receipt.recipient_actor_ref,
              target_forum_id: receipt.target_forum_id,
              target_post_id: receipt.target_post_id,
              target_topic_id: receipt.target_topic_id,
            },
          ]
        })

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    if (
      this.query.includes('FROM forum_receipts') &&
      this.query.includes('recipient_actor_ref = ?')
    ) {
      const actorRef = String(this.values[0])
      const rows = this.store.receipts.filter(
        item =>
          item.recipient_actor_ref === actorRef && item.archived_at === null,
      )

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM forum_notification_reads')) {
      const actorRef = String(this.values[0])
      const rows = this.store.notificationReads
        .filter(
          item => item.actor_ref === actorRef && item.archived_at === null,
        )
        .sort((left, right) => right.read_at.localeCompare(left.read_at))

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
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
        const forum = this.store.forums.find(
          forum => forum.id === item.forum_id,
        )

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

    if (this.query.includes('FROM forum_reports')) {
      const limit = Number(this.values[0] ?? 50)
      const rows = this.store.reports
        .filter(
          item =>
            (item.status === 'open' || item.status === 'reviewing') &&
            item.archived_at === null,
        )
        .slice(0, limit)

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    if (this.query.includes('FROM forum_topics')) {
      if (this.query.includes('forum.agentProfileActivity.topics')) {
        const actorRef = String(this.values[0])
        const limit = Number(this.values[1] ?? 12)
        const rows = this.store.topics
          .filter(item => {
            const forum = this.store.forums.find(f => f.id === item.forum_id)
            const actor = JSON.parse(item.actor_json) as { actorRef: string }

            return (
              actor.actorRef === actorRef &&
              item.archived_at === null &&
              (item.state === 'open' || item.state === 'locked') &&
              forum !== undefined &&
              forum.archived_at === null &&
              forum.visibility === 'public' &&
              forum.discoverability === 'listed'
            )
          })
          .sort(
            (left, right) =>
              right.created_at.localeCompare(left.created_at) ||
              right.id.localeCompare(left.id),
          )
          .slice(0, limit)
          .map(item => {
            const firstPost = this.store.posts.find(
              post =>
                post.id === item.first_post_id &&
                post.archived_at === null &&
                (post.state === 'visible' || post.state === 'edited'),
            )

            return {
              activity_id: item.id,
              created_at: item.created_at,
              first_post_receipt_refs_json:
                firstPost?.receipt_refs_json ?? '[]',
              state: item.state,
              title: item.title,
              topic_id: item.id,
              updated_at: item.updated_at,
            }
          })

        return Promise.resolve({ results: rows } as unknown as D1Result<T>)
      }

      if (this.query.includes("state = 'hidden'")) {
        const limit = Number(this.values[0] ?? 50)
        const rows = this.store.topics
          .filter(item => item.state === 'hidden' && item.archived_at === null)
          .slice(0, limit)

        return Promise.resolve({ results: rows } as unknown as D1Result<T>)
      }

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
      if (this.query.includes('forum.agentProfileActivity.posts')) {
        const actorRef = String(this.values[0])
        const limit = Number(this.values[1] ?? 12)
        const rows = this.store.posts
          .filter(item => {
            const topic = this.store.topics.find(t => t.id === item.topic_id)
            const forum = this.store.forums.find(f => f.id === item.forum_id)
            const actor = JSON.parse(item.actor_json) as { actorRef: string }

            return (
              actor.actorRef === actorRef &&
              item.archived_at === null &&
              (item.state === 'visible' || item.state === 'edited') &&
              topic !== undefined &&
              topic.archived_at === null &&
              (topic.state === 'open' || topic.state === 'locked') &&
              forum !== undefined &&
              forum.archived_at === null &&
              forum.visibility === 'public' &&
              forum.discoverability === 'listed'
            )
          })
          .sort(
            (left, right) =>
              right.created_at.localeCompare(left.created_at) ||
              right.id.localeCompare(left.id),
          )
          .slice(0, limit)
          .map(item => {
            const topic = this.store.topics.find(t => t.id === item.topic_id)

            return {
              activity_id: item.id,
              created_at: item.created_at,
              post_id: item.id,
              receipt_refs_json: item.receipt_refs_json,
              state: item.state,
              title: topic?.title ?? 'Forum topic',
              topic_id: item.topic_id,
              updated_at: item.updated_at,
            }
          })

        return Promise.resolve({ results: rows } as unknown as D1Result<T>)
      }

      if (
        this.query.includes('forum_posts.actor_ref = ?') &&
        this.query.includes('forum_posts.created_at >= ?')
      ) {
        const actorRef = String(this.values[0])
        const sinceIso = String(this.values[1])
        const limit = Number(this.values[2] ?? 100)
        const rows = this.store.posts
          .filter(item => {
            const actor = JSON.parse(item.actor_json) as { actorRef: string }

            return (
              actor.actorRef === actorRef &&
              item.created_at >= sinceIso &&
              item.archived_at === null &&
              item.state !== 'tombstoned'
            )
          })
          .sort((left, right) =>
            right.created_at.localeCompare(left.created_at),
          )
          .slice(0, limit)
          .map(item => ({
            body_text: item.body_text,
            created_at: item.created_at,
            id: item.id,
            idempotency_key: item.idempotency_key,
            post_number: item.post_number,
            state: item.state,
          }))

        return Promise.resolve({ results: rows } as unknown as D1Result<T>)
      }

      if (this.query.includes("state IN ('held_for_review', 'hidden')")) {
        const limit = Number(this.values[0] ?? 50)
        const rows = this.store.posts
          .filter(
            item =>
              (item.state === 'held_for_review' || item.state === 'hidden') &&
              item.archived_at === null,
          )
          .slice(0, limit)

        return Promise.resolve({ results: rows } as unknown as D1Result<T>)
      }

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
          const topic = this.store.topics.find(
            topic => topic.id === item.topic_id,
          )
          const forum = this.store.forums.find(
            forum => forum.id === item.forum_id,
          )

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

      if (
        this.query.includes('JOIN forum_topics') &&
        !this.query.includes('body_text LIKE ?')
      ) {
        const values = this.values
        const hasCursor = this.query.includes('forum_posts.created_at < ?')
        const hasForumFilter = this.query.includes(
          '(forum_forums.id = ? OR forum_forums.slug = ?)',
        )
        const hasTopicFilter = this.query.includes('forum_topics.id = ?')
        const cursorCreatedAt = hasCursor ? String(values[0]) : null
        const cursorPostId = hasCursor ? String(values[2]) : null
        const forumFilterIndex = hasCursor ? 3 : 0
        const forumRef = hasForumFilter
          ? String(values[forumFilterIndex])
          : null
        const topicFilterIndex = forumFilterIndex + (hasForumFilter ? 2 : 0)
        const topicId = hasTopicFilter ? String(values[topicFilterIndex]) : null
        const limit = Number(values[values.length - 1] ?? 50)
        const listedOnly = this.query.includes(
          "forum_forums.discoverability = 'listed'",
        )
        const rows = this.store.posts
          .filter(item => {
            const topic = this.store.topics.find(t => t.id === item.topic_id)
            const forum = this.store.forums.find(f => f.id === item.forum_id)
            const afterCursor =
              cursorCreatedAt === null || cursorPostId === null
                ? true
                : item.created_at < cursorCreatedAt ||
                  (item.created_at === cursorCreatedAt &&
                    item.id < cursorPostId)

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
              (forumRef === null ||
                forum.id === forumRef ||
                forum.slug === forumRef) &&
              (topicId === null || topic.id === topicId) &&
              afterCursor
            )
          })
          .sort(
            (left, right) =>
              right.created_at.localeCompare(left.created_at) ||
              right.id.localeCompare(left.id),
          )
          .slice(0, limit)

        return Promise.resolve({
          results: rows.map(row => {
            const topic = this.store.topics.find(t => t.id === row.topic_id)!
            const forum = this.store.forums.find(f => f.id === row.forum_id)!

            return {
              ...row,
              forum_archived_at: forum.archived_at,
              forum_board_id: forum.board_id,
              forum_category_id: forum.category_id,
              forum_description_ref: forum.description_ref,
              forum_discoverability: forum.discoverability,
              forum_latest_post_id: forum.latest_post_id,
              forum_latest_topic_id: forum.latest_topic_id,
              forum_locked: forum.locked,
              forum_post_count: forum.post_count,
              forum_public_projection_json: forum.public_projection_json,
              forum_slug: forum.slug,
              forum_title: forum.title,
              forum_topic_count: forum.topic_count,
              forum_visibility: forum.visibility,
              topic_actor_json: topic.actor_json,
              topic_archived_at: topic.archived_at,
              topic_created_at: topic.created_at,
              topic_first_post_id: topic.first_post_id,
              topic_forum_id: topic.forum_id,
              topic_id: topic.id,
              topic_latest_post_id: topic.latest_post_id,
              topic_pin_state: topic.pin_state,
              topic_post_count: topic.post_count,
              topic_public_projection_json: topic.public_projection_json,
              topic_score_ref: topic.score_ref,
              topic_slug: topic.slug,
              topic_state: topic.state,
              topic_title: topic.title,
              topic_updated_at: topic.updated_at,
            }
          }),
        } as unknown as D1Result<T>)
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
      // Mirror production: the topic-detail projection only includes live
      // (visible/edited) posts. Tombstoned posts are excluded so a deleted
      // post never renders in the thread.
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

    // The reliable-tip payments ledger (pay_ins/pay_in_legs) is empty in
    // this route fixture; ladder reads project no rows (#4753).
    if (this.query.includes('FROM pay_ins')) {
      return Promise.resolve({ results: [] } as unknown as D1Result<T>)
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

const storeArraySnapshot = (
  store: ForumRouteStore,
): Map<string, ReadonlyArray<unknown>> => {
  const snapshot = new Map<string, ReadonlyArray<unknown>>()

  for (const key of Object.keys(store)) {
    const value = (store as unknown as Record<string, unknown>)[key]

    if (Array.isArray(value)) {
      snapshot.set(key, [...value])
    }
  }

  return snapshot
}

const restoreStoreArrays = (
  store: ForumRouteStore,
  snapshot: Map<string, ReadonlyArray<unknown>>,
): void => {
  for (const [key, rows] of snapshot) {
    const target = (store as unknown as Record<string, Array<unknown>>)[key]

    if (Array.isArray(target)) {
      target.length = 0
      target.push(...rows)
    }
  }
}

const forumRouteDb = (store: ForumRouteStore): D1Database => ({
  batch: (async (statements: ReadonlyArray<D1PreparedStatement>) => {
    const snapshot = storeArraySnapshot(store)
    const results: Array<unknown> = []

    try {
      for (const statement of statements) {
        results.push(await statement.run())
      }
    } catch (error) {
      restoreStoreArrays(store, snapshot)
      throw error
    }

    return results
  }) as D1Database['batch'],
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new ForumRouteStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const testAgentStore = (profileMetadata: Record<string, unknown> = {}) => ({
  createAgentRegistration: () => Promise.resolve(),
  findAgentByTokenHash: () =>
    Promise.resolve({
      credentialId: 'credential-route-test',
      profileMetadataJson: JSON.stringify(profileMetadata),
      tokenPrefix: 'oa_agent_route',
      user: {
        avatarUrl: null,
        createdAt: '2026-06-05T20:00:00.000Z',
        displayName: 'Route Test Agent',
        id: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
        kind: 'agent' as const,
        primaryEmail: 'agent@example.com',
        status: 'active' as const,
        updatedAt: '2026-06-05T20:00:00.000Z',
      },
    }),
  touchAgentCredential: () => Promise.resolve(),
  updateAgentDisplayName: () => Promise.resolve(0),
})

const verifiedPublicIdentityClaim: VerifiedPublicIdentityClaim = {
  agentClaimRef: 'agent_claim_route_test',
  claimRef: 'agent_x_claim_route_test',
  ownerUserId: 'github:route-owner',
  provider: 'x',
  receiptRef: 'agent_x_claim_receipt_route_test',
  state: 'verified',
  tweetRef: 'x_tweet:100',
  xAccountRef: 'x:routeowner',
}

const route = async (
  store: ForumRouteStore,
  path: string,
  options: Readonly<{
    hostedMdkClient?: ReturnType<typeof forumHostedMdkClient>
    agentClaimed?: boolean
    agentMetadata?: Record<string, unknown>
    body?: unknown
    headers?: HeadersInit
    moderator?: 'admin' | 'non_admin'
    method?: string
    productPromisesUnsupportedRequestIngest?: NonNullable<
      Parameters<typeof makeForumRoutes>[0]
    >['productPromisesUnsupportedRequestIngest']
    workRequestEscrowReserver?: NonNullable<
      Parameters<typeof makeForumRoutes>[0]
    >['forumWorkRequestEscrowReserver']
    workRequestRelayPublisher?: ReturnType<typeof fakeWorkRequestRelayPublisher>
    workRequestRelayUrl?: string
  }> = {},
) => {
  const init: RequestInit = {
    headers: {
      ...(options.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
      ...(options.headers ?? {}),
    },
    method: options.method ?? 'GET',
  }

  if (options.body !== undefined) {
    init.body = JSON.stringify(options.body)
  }

  const request = new Request(`https://openagents.com${path}`, init)
  const effect = makeForumRoutes({
    agentStore: testAgentStore(options.agentMetadata),
    ...(options.workRequestEscrowReserver === undefined
      ? {}
      : { forumWorkRequestEscrowReserver: options.workRequestEscrowReserver }),
    ...(options.workRequestRelayPublisher === undefined
      ? {}
      : { forumWorkRequestRelayPublisher: options.workRequestRelayPublisher }),
    ...(options.workRequestRelayUrl === undefined
      ? {}
      : { forumWorkRequestRelayUrl: options.workRequestRelayUrl }),
    hostedMdkClient: options.hostedMdkClient ?? forumHostedMdkClient(),
    l402SigningBoundary: forumL402SigningBoundary,
    makeId: () => store.nextId(),
    mdkWebhookConfig: {
      bindingRef: 'binding.forum.route.mdk.webhook',
      secret: forumMdkWebhookSecret,
      source: 'dashboard_standard_webhooks',
    },
    nowEpochMillis: () => 1_780_000_000_000,
    nowIso: () => '2026-06-05T20:00:00.000Z',
    ...(options.productPromisesUnsupportedRequestIngest === undefined
      ? {}
      : {
          productPromisesUnsupportedRequestIngest:
            options.productPromisesUnsupportedRequestIngest,
        }),
    publicIdentityClaimStore: {
      readVerifiedPublicIdentityForAgentUserId: () =>
        Promise.resolve(
          options.agentClaimed === false
            ? undefined
            : verifiedPublicIdentityClaim,
        ),
    },
    resolveModeratorActor: () =>
      Promise.resolve(
        options.moderator === undefined
          ? undefined
          : options.moderator === 'admin'
            ? {
                _tag: 'Moderator' as const,
                actor: {
                  displayName: 'Route Moderator',
                  operatorId: 'github:moderator',
                  slug: 'route-moderator',
                },
              }
            : {
                _tag: 'Forbidden' as const,
                reason:
                  'Forum moderation requires an OpenAgents admin session.',
              },
      ),
  }).routeForumRequest(request, forumRouteDb(store))

  if (effect === undefined) {
    throw new Error(`Forum route was not matched for ${path}.`)
  }

  return Effect.runPromise(effect)
}

type ForumIndexBody = Readonly<{
  forums: ReadonlyArray<Readonly<{ slug: string }>>
}>

describe('Forum routes', () => {
  test('hides void from default discovery and includes it with an explicit test flag', async () => {
    const store = new ForumRouteStore()
    const defaultResponse = await route(store, '/api/forum')
    const defaultBody = (await defaultResponse.json()) as ForumIndexBody
    const unauthorizedTestResponse = await route(store, '/api/forum?test=void')
    const testResponse = await route(store, '/api/forum?test=void')
    const authedTestResponse = await route(store, '/api/forum?test=void', {
      headers: {
        authorization: 'Bearer oa_agent_route_test',
      },
    })
    const testBody = (await authedTestResponse.json()) as ForumIndexBody

    expect(defaultResponse.status).toBe(200)
    expect(unauthorizedTestResponse.status).toBe(401)
    expect(defaultBody.forums.map(forum => forum.slug)).toStrictEqual([
      'site-builder-help',
      'artanis',
      'product-promises',
      'work-requests',
    ])
    expect(testResponse.status).toBe(401)
    expect(testBody.forums.map(forum => forum.slug).sort()).toStrictEqual([
      'artanis',
      'product-promises',
      'site-builder-help',
      'void',
      'work-requests',
    ])
  })

  test('discovers Product Promises as the public product report forum', async () => {
    const store = new ForumRouteStore()
    const ingested: Array<{
      bodyText: string
      firstPostId: string
      forumId: string
      sourceRef: string
      title: string
      topicId: string
    }> = []
    const forum = await route(store, '/api/forum/forums/product-promises')
    const topic = await route(
      store,
      '/api/forum/forums/product-promises/topics',
      {
        body: {
          bodyText:
            'This product promise report is public-safe and cites a visible claim mismatch.',
          title: 'Promise report: example mismatch',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'product-promises-agent-topic-create-1',
        },
        method: 'POST',
        productPromisesUnsupportedRequestIngest: input => {
          ingested.push(input)
          return Promise.resolve()
        },
      },
    )

    await expect(forum.json()).resolves.toMatchObject({
      discoverability: 'listed',
      slug: 'product-promises',
      title: 'Product Promises',
    })
    expect(topic.status).toBe(201)
    await expect(topic.json()).resolves.toMatchObject({
      topic: {
        slug: 'promise-report-example-mismatch',
        title: 'Promise report: example mismatch',
      },
    })
    expect(ingested).toStrictEqual([
      {
        bodyText:
          'This product promise report is public-safe and cites a visible claim mismatch.',
        firstPostId: 'aaaaaaaa-1111-4111-8111-000000000002',
        forumId: '99999999-3333-4333-8333-999999999999',
        sourceRef: 'forum.topic:aaaaaaaa-1111-4111-8111-000000000001',
        title: 'Promise report: example mismatch',
        topicId: 'aaaaaaaa-1111-4111-8111-000000000001',
      },
    ])

    const retry = await route(
      store,
      '/api/forum/forums/product-promises/topics',
      {
        body: {
          bodyText:
            'This product promise report is public-safe and cites a visible claim mismatch.',
          title: 'Promise report: example mismatch',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'product-promises-agent-topic-create-1',
        },
        method: 'POST',
        productPromisesUnsupportedRequestIngest: input => {
          ingested.push(input)
          return Promise.resolve()
        },
      },
    )

    expect(retry.status).toBe(200)
    await expect(retry.json()).resolves.toMatchObject({ idempotent: true })
    expect(ingested).toHaveLength(2)

    const otherForumTopic = await route(
      store,
      '/api/forum/forums/site-builder-help/topics',
      {
        body: {
          bodyText: 'Question for the site builder forum.',
          title: 'Site builder question',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'site-builder-topic-create-1',
        },
        method: 'POST',
        productPromisesUnsupportedRequestIngest: input => {
          ingested.push(input)
          return Promise.resolve()
        },
      },
    )

    expect(otherForumTopic.status).toBe(201)
    expect(ingested).toHaveLength(2)
  })

  test('keeps Product Promises posting available if unsupported-request ingestion fails', async () => {
    const store = new ForumRouteStore()
    const topic = await route(
      store,
      '/api/forum/forums/product-promises/topics',
      {
        body: {
          bodyText:
            'This Product Promises post should still publish if the ledger is temporarily unavailable.',
          title: 'Promise report: ledger outage',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'product-promises-agent-topic-create-fail-soft',
        },
        method: 'POST',
        productPromisesUnsupportedRequestIngest: () =>
          Promise.reject(new Error('ledger unavailable')),
      },
    )

    expect(topic.status).toBe(201)
    await expect(topic.json()).resolves.toMatchObject({
      topic: {
        slug: 'promise-report-ledger-outage',
        title: 'Promise report: ledger outage',
      },
    })
  })

  test('creates Forum work requests as NIP-LBR relay jobs with idempotent linkage', async () => {
    const store = new ForumRouteStore()
    const captured: Array<CapturedWorkRequestRelayPublish> = []
    const publisher = fakeWorkRequestRelayPublisher(captured)
    const request = {
      body: {
        budgetSats: 2_500,
        deadlineRef: 'deadline.public.lbr.20260612',
        objectiveRef: 'objective.public.openagents.forum_bridge_smoke',
        repositoryRefs: ['repo.public.openagents'],
        requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
        title: 'Forum bridge smoke request',
        verificationCommandRef: 'command.public.bun_vitest_forum_work_requests',
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'work-request-create-1',
      },
      method: 'POST',
      workRequestRelayPublisher: publisher,
      workRequestRelayUrl: 'wss://relay.test.openagents.dev',
    }
    const created = await route(store, '/api/forum/work-requests', request)
    const retry = await route(store, '/api/forum/work-requests', request)
    const conflict = await route(store, '/api/forum/work-requests', {
      ...request,
      body: {
        ...request.body,
        objectiveRef: 'objective.public.openagents.changed',
      },
    })
    const list = await route(store, '/api/forum/work-requests')

    expect(created.status).toBe(201)
    expect(retry.status).toBe(200)
    expect(conflict.status).toBe(409)
    expect(captured).toHaveLength(1)
    expect(captured[0]?.draft.kind).toBe(5934)
    expect(captured[0]?.draft.content).toBe('')
    expect(captured[0]?.draft.tags).toContainEqual([
      'param',
      'lbr_objective_ref',
      'objective.public.openagents.forum_bridge_smoke',
    ])
    await expect(created.json()).resolves.toMatchObject({
      firstPost: {
        bodyText: expect.stringContaining(
          'Job event ref: nostr.event.0000000000000000000000000000000000000000000000000000000000000001',
        ),
      },
      idempotent: false,
      relayLink: {
        jobEventId:
          '0000000000000000000000000000000000000000000000000000000000000001',
        jobEventKind: 5934,
      },
      topic: {
        slug: 'forum-bridge-smoke-request',
        title: 'Forum bridge smoke request',
      },
      workRequest: {
        budgetSats: 2500,
        jobEventKind: 5934,
        jobResultKind: 6934,
        objectiveRef: 'objective.public.openagents.forum_bridge_smoke',
        state: 'open',
      },
    })
    await expect(retry.json()).resolves.toMatchObject({
      idempotent: true,
      relayLink: {
        jobEventId:
          '0000000000000000000000000000000000000000000000000000000000000001',
      },
    })
    await expect(list.json()).resolves.toMatchObject({
      generatedAt: expect.any(String),
      maxStalenessSeconds: 0,
      staleness: {
        composition: 'live_at_read',
        contractVersion: 'projection_staleness.v1',
        maxStalenessSeconds: 0,
        rebuildsOn: expect.arrayContaining([
          'forum_work_request_created',
          'forum_work_request_lifecycle_recorded',
          'forum_work_request_quote_recorded',
        ]),
      },
      workRequests: [
        {
          objectiveRef: 'objective.public.openagents.forum_bridge_smoke',
          state: 'open',
        },
      ],
    })
    expect(store.workRequests).toHaveLength(1)
    expect(store.workRequestRelayLinks).toHaveLength(1)
  })

  test('rejects invalid or unsafe Forum work request material before persistence', async () => {
    const store = new ForumRouteStore()
    const captured: Array<CapturedWorkRequestRelayPublish> = []
    const base = {
      budgetSats: 2_500,
      deadlineRef: 'deadline.public.lbr.20260612',
      objectiveRef: 'objective.public.openagents.safe',
      repositoryRefs: ['repo.public.openagents'],
      requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
      title: 'Safe work request',
      verificationCommandRef: 'command.public.bun_test',
    }
    const invalid = await route(store, '/api/forum/work-requests', {
      body: { ...base, budgetSats: 0 },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'work-request-invalid-1',
      },
      method: 'POST',
      workRequestRelayPublisher: fakeWorkRequestRelayPublisher(captured),
    })
    const unsafe = await route(store, '/api/forum/work-requests', {
      body: {
        ...base,
        rawPrompt: 'fix this using OPENAI_API_KEY=secret',
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'work-request-unsafe-1',
      },
      method: 'POST',
      workRequestRelayPublisher: fakeWorkRequestRelayPublisher(captured),
    })

    expect(invalid.status).toBe(400)
    expect(unsafe.status).toBe(400)
    await expect(unsafe.json()).resolves.toMatchObject({
      error: 'bad_request',
      reason: expect.stringContaining('public refs only'),
    })
    expect(captured).toHaveLength(0)
    expect(store.workRequests).toHaveLength(0)
    expect(store.topics.some(topic => topic.forum_id === store.forums[4]?.id)).toBe(
      false,
    )
  })

  test('records Forum work request lifecycle updates as idempotent topic replies', async () => {
    const store = new ForumRouteStore()
    const captured: Array<CapturedWorkRequestRelayPublish> = []
    const created = await route(store, '/api/forum/work-requests', {
      body: {
        budgetSats: 1_000,
        deadlineRef: 'deadline.public.lbr.20260612',
        objectiveRef: 'objective.public.openagents.lifecycle',
        repositoryRefs: ['repo.public.openagents'],
        requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
        title: 'Lifecycle request',
        verificationCommandRef: 'command.public.bun_lifecycle_test',
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'work-request-lifecycle-root',
      },
      method: 'POST',
      workRequestRelayPublisher: fakeWorkRequestRelayPublisher(captured),
    })
    const createdBody = (await created.json()) as Readonly<{
      topic: Readonly<{ postCount: number }>
      workRequest: Readonly<{ workRequestId: string }>
    }>
    const lifecycleRequest = {
      body: {
        lifecycleKind: 'quote_received',
        receiptRef: 'receipt.public.lbr.quote_1',
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'work-request-lifecycle-quote-1',
      },
      method: 'POST',
    }
    const lifecycle = await route(
      store,
      `/api/forum/work-requests/${createdBody.workRequest.workRequestId}/lifecycle-posts`,
      lifecycleRequest,
    )
    const retry = await route(
      store,
      `/api/forum/work-requests/${createdBody.workRequest.workRequestId}/lifecycle-posts`,
      lifecycleRequest,
    )

    expect(created.status).toBe(201)
    expect(createdBody.topic.postCount).toBe(1)
    expect(lifecycle.status).toBe(201)
    expect(retry.status).toBe(200)
    await expect(lifecycle.json()).resolves.toMatchObject({
      idempotent: false,
      lifecyclePost: {
        lifecycleKind: 'quote_received',
        receiptRef: 'receipt.public.lbr.quote_1',
      },
      post: {
        bodyText: expect.stringContaining('Receipt ref: receipt.public.lbr.quote_1'),
        postNumber: 2,
      },
      workRequest: { quoteCount: 1, state: 'quote_received' },
    })
    await expect(retry.json()).resolves.toMatchObject({
      idempotent: true,
      post: { postNumber: 2 },
      workRequest: { quoteCount: 1, state: 'quote_received' },
    })
    expect(store.workRequestLifecyclePosts).toHaveLength(1)
    expect(
      store.posts.filter(post => post.topic_id === store.workRequests[0]?.topic_id),
    ).toHaveLength(2)
  })

  test('lists and accepts Forum work-request offers with escrow reserve enforcement', async () => {
    const store = new ForumRouteStore()
    const captured: Array<CapturedWorkRequestRelayPublish> = []
    const created = await route(store, '/api/forum/work-requests', {
      body: {
        budgetSats: 2_000,
        deadlineRef: 'deadline.public.lbr.20260612',
        objectiveRef: 'objective.public.openagents.requester_accept',
        repositoryRefs: ['repo.public.openagents'],
        requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
        title: 'Requester accept request',
        verificationCommandRef: 'command.public.bun_requester_accept',
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'work-request-accept-root',
      },
      method: 'POST',
      workRequestRelayPublisher: fakeWorkRequestRelayPublisher(captured),
    })
    const createdBody = (await created.json()) as Readonly<{
      workRequest: Readonly<{ workRequestId: string }>
    }>
    const workRequestId = createdBody.workRequest.workRequestId

    store.workRequestOffers.push(
      {
        amount_msats: 1_500_000,
        amount_sats: 1_500,
        archived_at: null,
        capability_refs_json: JSON.stringify([
          'capability.pylon.local_claude_agent',
        ]),
        created_at: '2026-06-05T20:01:00.000Z',
        id: 'offer_route_1',
        provider_actor_ref: 'agent:provider-one',
        public_projection_json: '{}',
        quote_ref: 'quote.public.route.one',
        relay_event_ref: 'nostr.event.' + '1'.repeat(64),
        state: 'offered',
        updated_at: '2026-06-05T20:01:00.000Z',
        work_request_id: workRequestId,
      },
      {
        amount_msats: 1_600_000,
        amount_sats: 1_600,
        archived_at: null,
        capability_refs_json: JSON.stringify([
          'capability.pylon.local_claude_agent',
        ]),
        created_at: '2026-06-05T20:02:00.000Z',
        id: 'offer_route_2',
        provider_actor_ref: 'agent:provider-two',
        public_projection_json: '{}',
        quote_ref: 'quote.public.route.two',
        relay_event_ref: 'nostr.event.' + '2'.repeat(64),
        state: 'offered',
        updated_at: '2026-06-05T20:02:00.000Z',
        work_request_id: workRequestId,
      },
    )

    const reservedInputs: unknown[] = []
    const escrowReserver: NonNullable<
      Parameters<typeof makeForumRoutes>[0]
    >['forumWorkRequestEscrowReserver'] = async input => {
      reservedInputs.push(input)
      return {
        escrow: {
          amountMsat: input.amountMsat,
          createdAt: input.nowIso,
          escrowId: input.escrowId,
          fundingSource: 'ledger_balance',
          idempotencyKey: input.idempotencyKey,
          jobEventId: input.jobEventId,
          providerActorRef: null,
          publicProjection: {
            amountMsat: input.amountMsat,
            escrowRef: `labor_escrow.public.${input.escrowId}`,
            evidenceRef: 'nostr.event.' + input.jobEventId,
            jobEventRef: 'nostr.event.' + input.jobEventId,
            providerActorRef: null,
            receiptRef: input.reserveReceiptRef,
            requesterActorRef: input.requesterActorRef,
            stateAfter: 'reserved',
            transitionKind: 'reserve',
            workRequestRef: `work_request.public.${input.workRequestId}`,
          },
          requesterActorRef: input.requesterActorRef,
          reserveReceiptRef: input.reserveReceiptRef,
          releaseReceiptRef: null,
          refundReceiptRef: null,
          forfeitReceiptRef: null,
          forfeitDestination: null,
          forfeitDestinationActorRef: null,
          forfeitConditionRef: null,
          state: 'reserved',
          updatedAt: input.nowIso,
          workRequestId: input.workRequestId,
        },
        ok: true,
        reserveReceiptRef: input.reserveReceiptRef,
      }
    }

    const offers = await route(
      store,
      `/api/forum/work-requests/${workRequestId}/offers`,
    )
    const accepted = await route(
      store,
      `/api/forum/work-requests/${workRequestId}/acceptances`,
      {
        body: { quoteRef: 'quote.public.route.one' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'work-request-accept-quote-one',
        },
        method: 'POST',
        workRequestEscrowReserver: escrowReserver,
      },
    )
    const retry = await route(
      store,
      `/api/forum/work-requests/${workRequestId}/acceptances`,
      {
        body: { quoteRef: 'quote.public.route.one' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'work-request-accept-quote-one',
        },
        method: 'POST',
        workRequestEscrowReserver: escrowReserver,
      },
    )
    const unauthenticatedRetry = await route(
      store,
      `/api/forum/work-requests/${workRequestId}/acceptances`,
      {
        body: { quoteRef: 'quote.public.route.one' },
        headers: {
          'idempotency-key': 'work-request-accept-quote-one',
        },
        method: 'POST',
        workRequestEscrowReserver: escrowReserver,
      },
    )
    const doubleAccept = await route(
      store,
      `/api/forum/work-requests/${workRequestId}/acceptances`,
      {
        body: { quoteRef: 'quote.public.route.two' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'work-request-accept-quote-two',
        },
        method: 'POST',
        workRequestEscrowReserver: escrowReserver,
      },
    )
    const status = await route(store, `/api/forum/work-requests/${workRequestId}`)

    expect(offers.status).toBe(200)
    await expect(offers.json()).resolves.toMatchObject({
      offers: [
        { quoteRef: 'quote.public.route.two' },
        { quoteRef: 'quote.public.route.one' },
      ],
    })
    expect(accepted.status).toBe(201)
    expect(retry.status).toBe(200)
    expect(unauthenticatedRetry.status).toBe(401)
    expect(doubleAccept.status).toBe(409)
    expect(reservedInputs).toHaveLength(1)
    await expect(accepted.json()).resolves.toMatchObject({
      acceptance: {
        providerActorRef: 'agent:provider-one',
        quoteRef: 'quote.public.route.one',
      },
      acceptedOffer: { quoteRef: 'quote.public.route.one' },
      escrowState: {
        reserveReceiptRef: expect.stringContaining(
          'receipt.labor_escrow.reserve',
        ),
        state: 'reserved',
      },
      idempotent: false,
      workRequest: { state: 'quote_accepted' },
    })
    await expect(retry.json()).resolves.toMatchObject({ idempotent: true })
    await expect(doubleAccept.json()).resolves.toMatchObject({
      error: 'quote_already_accepted',
    })
    await expect(status.json()).resolves.toMatchObject({
      acceptance: { quoteRef: 'quote.public.route.one' },
      offers: expect.arrayContaining([
        expect.objectContaining({
          quoteRef: 'quote.public.route.one',
          state: 'accepted',
        }),
        expect.objectContaining({
          quoteRef: 'quote.public.route.two',
          state: 'rejected',
        }),
      ]),
    })
  })

  test('refuses work-request quote acceptance when escrow reserve lacks balance', async () => {
    const store = new ForumRouteStore()
    const captured: Array<CapturedWorkRequestRelayPublish> = []
    const created = await route(store, '/api/forum/work-requests', {
      body: {
        budgetSats: 2_000,
        deadlineRef: 'deadline.public.lbr.20260612',
        objectiveRef: 'objective.public.openagents.requester_balance',
        repositoryRefs: ['repo.public.openagents'],
        requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
        title: 'Requester balance request',
        verificationCommandRef: 'command.public.bun_requester_balance',
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'work-request-balance-root',
      },
      method: 'POST',
      workRequestRelayPublisher: fakeWorkRequestRelayPublisher(captured),
    })
    const createdBody = (await created.json()) as Readonly<{
      workRequest: Readonly<{ workRequestId: string }>
    }>
    const workRequestId = createdBody.workRequest.workRequestId
    store.workRequestOffers.push({
      amount_msats: 1_500_000,
      amount_sats: 1_500,
      archived_at: null,
      capability_refs_json: JSON.stringify([
        'capability.pylon.local_claude_agent',
      ]),
      created_at: '2026-06-05T20:01:00.000Z',
      id: 'offer_balance_1',
      provider_actor_ref: 'agent:provider-one',
      public_projection_json: '{}',
      quote_ref: 'quote.public.balance.one',
      relay_event_ref: 'nostr.event.' + '3'.repeat(64),
      state: 'offered',
      updated_at: '2026-06-05T20:01:00.000Z',
      work_request_id: workRequestId,
    })

    const refused = await route(
      store,
      `/api/forum/work-requests/${workRequestId}/acceptances`,
      {
        body: { quoteRef: 'quote.public.balance.one' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'work-request-balance-accept',
        },
        method: 'POST',
        workRequestEscrowReserver: async () => ({
          availableMsat: 0,
          ok: false,
          reason: 'insufficient_available_balance',
        }),
      },
    )

    expect(refused.status).toBe(409)
    await expect(refused.json()).resolves.toMatchObject({
      error: 'labor_escrow_refused',
      reason: 'insufficient_available_balance',
    })
    expect(store.workRequestAcceptances).toHaveLength(0)
    expect(store.workRequestOffers[0]?.state).toBe('offered')
  })

  test('ingests relay-native NIP-LBR requests into twin Forum work-request topics', async () => {
    const store = new ForumRouteStore()
    const lbr = makeLbrAgenticCodingRequest({
      bidMsats: 3_000_000,
      deadline: 'deadline.public.lbr.20260612',
      objectiveRef: 'objective.public.openagents.relay_native',
      relays: ['wss://relay.test.openagents.dev'],
      repositoryRefs: ['repo.public.openagents'],
      requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
      verificationCommandRef: 'command.public.bun_relay_native',
    })
    const draft = lbrAgenticCodingRequestToDraft(lbr)
    const event = {
      content: draft.content,
      created_at: 1_781_107_200,
      id: 'd'.repeat(64),
      kind: draft.kind,
      pubkey: 'e'.repeat(64),
      sig: 'f'.repeat(128),
      tags: draft.tags,
    }
    const request = {
      body: { event, title: 'Relay native request' },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'relay-native-work-request-1',
      },
      method: 'POST',
      workRequestRelayUrl: 'wss://relay.test.openagents.dev',
    }
    const created = await route(
      store,
      '/api/forum/work-requests/relay-events',
      request,
    )
    const retry = await route(store, '/api/forum/work-requests/relay-events', {
      ...request,
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'relay-native-work-request-2',
      },
    })

    expect(created.status).toBe(201)
    expect(retry.status).toBe(200)
    await expect(created.json()).resolves.toMatchObject({
      idempotent: false,
      relayLink: { jobEventId: 'd'.repeat(64), jobEventKind: 5934 },
      topic: { title: 'Relay native request' },
      workRequest: {
        jobEventId: 'd'.repeat(64),
        objectiveRef: 'objective.public.openagents.relay_native',
      },
    })
    await expect(retry.json()).resolves.toMatchObject({
      idempotent: true,
      relayLink: { jobEventId: 'd'.repeat(64) },
    })
    expect(store.workRequests).toHaveLength(1)
    expect(store.workRequestRelayLinks[0]?.topic_id).toBe(
      store.workRequests[0]?.topic_id,
    )
  })

  test('discovers Artanis canonical topics and keeps moderation operator-only', async () => {
    const store = new ForumRouteStore()
    const forum = await route(store, '/api/forum/forums/artanis')
    const topics = await route(store, '/api/forum/forums/artanis/topics')
    const topicDetail = await route(
      store,
      '/api/forum/topics/88888888-4001-4001-8001-888888888888',
    )
    const agentTopic = await route(store, '/api/forum/forums/artanis/topics', {
      body: {
        bodyText: 'Registered agent smoke post for Artanis coordination.',
        title: 'Registered agent Artanis smoke',
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'artanis-agent-topic-create-1',
      },
      method: 'POST',
    })
    const agentModeration = await route(
      store,
      '/api/forum/moderation/topics/88888888-4001-4001-8001-888888888888/lock',
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'artanis-agent-moderation-1',
        },
        method: 'POST',
      },
    )
    const moderatorLock = await route(
      store,
      '/api/forum/moderation/topics/88888888-4001-4001-8001-888888888888/lock',
      {
        headers: {
          'idempotency-key': 'artanis-moderator-lock-1',
        },
        method: 'POST',
        moderator: 'admin',
      },
    )
    const topicBody = (await topics.json()) as Readonly<{
      topics: ReadonlyArray<Readonly<{ pinState: string; title: string }>>
    }>

    await expect(forum.json()).resolves.toMatchObject({
      discoverability: 'listed',
      slug: 'artanis',
      title: 'Artanis',
    })
    expect(topicBody.topics.map(topic => topic.title)).toEqual([
      'Operator questions',
      'Resource modes',
      'Bitcoin accounting and rewards',
      'Work routing and accepted outcomes',
      'Pylon release work log',
      'Model Lab',
      'Pylon campaign status',
      'Artanis status',
    ])
    expect(topicBody.topics[0]?.pinState).toBe('sticky')
    expect(topicBody.topics.at(-1)?.pinState).toBe('announcement')
    await expect(topicDetail.json()).resolves.toMatchObject({
      posts: [
        {
          author: { actorRef: 'agent:agent_artanis', displayName: 'Artanis' },
          bodyText: expect.stringContaining('Canonical status thread'),
          postNumber: 1,
        },
      ],
      topic: {
        slug: 'artanis-status',
        title: 'Artanis status',
      },
    })
    expect(agentTopic.status).toBe(201)
    await expect(agentTopic.json()).resolves.toMatchObject({
      topic: {
        slug: 'registered-agent-artanis-smoke',
        title: 'Registered agent Artanis smoke',
      },
    })
    expect(agentModeration.status).toBe(401)
    expect(moderatorLock.status).toBe(201)
  })

  test('reads exact forum, topic list, topic detail, and post detail routes', async () => {
    const store = new ForumRouteStore()
    store.posts.push({
      actor_json: actorJson,
      archived_at: null,
      body_text: 'Seed route-test reply body.',
      content_ref: 'content.forum.route_test.reply',
      created_at: '2026-06-05T20:01:00.000Z',
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: '77777777-7777-4777-8777-777777777777',
      idempotency_key: 'seed-post-reply',
      parent_post_id: '66666666-6666-4666-8666-666666666666',
      post_number: 2,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '55555555-5555-4555-8555-555555555555',
      updated_at: '2026-06-05T20:01:00.000Z',
    })
    const voidForum = await route(store, '/api/forum/forums/void')
    const topics = await route(
      store,
      '/api/forum/forums/site-builder-help/topics',
    )
    const topic = await route(
      store,
      '/api/forum/topics/55555555-5555-4555-8555-555555555555',
    )
    const topicNewestFirst = await route(
      store,
      '/api/forum/topics/55555555-5555-4555-8555-555555555555?sortDir=desc',
    )
    const topicNewestFirstPhpbbAlias = await route(
      store,
      '/api/forum/topics/55555555-5555-4555-8555-555555555555?sd=d',
    )
    const malformedTopicSort = await route(
      store,
      '/api/forum/topics/55555555-5555-4555-8555-555555555555?sortDir=new',
    )
    const post = await route(
      store,
      '/api/forum/posts/66666666-6666-4666-8666-666666666666',
    )

    await expect(voidForum.json()).resolves.toMatchObject({
      discoverability: 'unlisted',
      slug: 'void',
    })
    await expect(topics.json()).resolves.toMatchObject({
      topics: [{ slug: 'first-topic' }],
    })
    await expect(topic.json()).resolves.toMatchObject({
      posts: [{ postNumber: 1 }, { postNumber: 2 }],
      topic: { slug: 'first-topic' },
    })
    await expect(topicNewestFirst.json()).resolves.toMatchObject({
      posts: [{ postNumber: 2 }, { postNumber: 1 }],
      topic: { slug: 'first-topic' },
    })
    await expect(topicNewestFirstPhpbbAlias.json()).resolves.toMatchObject({
      posts: [{ postNumber: 2 }, { postNumber: 1 }],
      topic: { slug: 'first-topic' },
    })
    expect(malformedTopicSort.status).toBe(400)
    await expect(malformedTopicSort.json()).resolves.toMatchObject({
      error: 'bad_request',
      reason: 'sortDir must be asc or desc',
    })
    await expect(post.json()).resolves.toMatchObject({
      containingTopicId: '55555555-5555-4555-8555-555555555555',
      post: {
        bodyText: 'Seed route-test body.',
        postNumber: 1,
        tipRecipientReadiness: {
          blockerRef: 'blocker.public.forum_tip_recipient.wallet_missing',
          providerClass: null,
          state: 'missing',
          tippingAvailable: false,
        },
      },
    })
  })

  test('projects prosilver display metadata without private internals', async () => {
    const store = new ForumRouteStore()
    const indexResponse = await route(store, '/api/forum')
    const topicsResponse = await route(
      store,
      '/api/forum/forums/site-builder-help/topics',
    )
    const topicResponse = await route(
      store,
      '/api/forum/topics/55555555-5555-4555-8555-555555555555',
    )
    const indexBody = (await indexResponse.json()) as Readonly<{
      forums: ReadonlyArray<
        Readonly<{
          capabilities: Readonly<{ canModerate: boolean; canReply: boolean }>
          category: Readonly<{ slug: string; title: string }>
          description: string | null
          lastPost: Readonly<{
            author: Readonly<{ displayName: string }>
            permalink: string
            title: string
          }> | null
          slug: string
        }>
      >
    }>
    const topicsBody = (await topicsResponse.json()) as Readonly<{
      topics: ReadonlyArray<
        Readonly<{
          capabilities: Readonly<{
            canBookmark: boolean
            canModerate: boolean
            canReply: boolean
          }>
          lastPost: Readonly<{ permalink: string; title: string }> | null
          replyCount: number
          slug: string
          topicType: string
          viewCount: number
        }>
      >
    }>
    const topicBody = (await topicResponse.json()) as Readonly<{
      posts: ReadonlyArray<
        Readonly<{
          authorProfile: Readonly<{
            publicUrl: string
            roleLabel: string
          }>
          capabilities: Readonly<{
            canEdit: boolean
            canModerate: boolean
            canQuote: boolean
            canTip: boolean
          }>
          permalink: string
          subject: string | null
        }>
      >
      topic: Readonly<{
        lastPost: Readonly<{ permalink: string; title: string }> | null
        replyCount: number
        viewCount: number
      }>
    }>

    const siteForum = indexBody.forums.find(
      forum => forum.slug === 'site-builder-help',
    )
    const firstTopic = topicsBody.topics.find(
      topic => topic.slug === 'first-topic',
    )
    const firstPost = topicBody.posts[0]
    const publicPayload = JSON.stringify({
      indexBody,
      topicBody,
      topicsBody,
    })

    expect(siteForum).toMatchObject({
      capabilities: { canModerate: false, canReply: true },
      category: { slug: 'sites', title: 'Sites' },
      description: 'Site builder help.',
      lastPost: {
        author: { displayName: 'Route Test' },
        permalink:
          'https://openagents.com/forum/t/55555555-5555-4555-8555-555555555555#post-66666666-6666-4666-8666-666666666666',
        title: 'First Topic',
      },
    })
    expect(firstTopic).toMatchObject({
      capabilities: {
        canBookmark: true,
        canModerate: false,
        canReply: true,
      },
      lastPost: {
        permalink:
          'https://openagents.com/forum/t/55555555-5555-4555-8555-555555555555#post-66666666-6666-4666-8666-666666666666',
        title: 'First Topic',
      },
      replyCount: 0,
      topicType: 'normal',
      viewCount: 0,
    })
    expect(topicBody.topic).toMatchObject({
      lastPost: { title: 'First Topic' },
      replyCount: 0,
      viewCount: 0,
    })
    expect(firstPost).toMatchObject({
      authorProfile: {
        publicUrl:
          'https://openagents.com/forum/u/aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa/route-test',
        roleLabel: 'Registered agent',
      },
      capabilities: {
        canEdit: false,
        canModerate: false,
        canQuote: true,
        canTip: false,
      },
      permalink:
        'https://openagents.com/forum/t/55555555-5555-4555-8555-555555555555#post-66666666-6666-4666-8666-666666666666',
      subject: 'First Topic',
    })
    expect(publicPayload).not.toContain('wallet_ref')
    expect(publicPayload).not.toContain('provider_ref')
    expect(publicPayload).not.toContain('redacted_evidence_ref')
    expect(publicPayload).not.toContain('moderator_actor_ref')
    expect(publicPayload).not.toContain('payment_event_id')
  })

  test('exposes ready tip-recipient status on post detail without wallet material', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    const response = await route(
      store,
      '/api/forum/posts/66666666-6666-4666-8666-666666666666',
    )
    const body = (await response.json()) as {
      attemptId: string
      receipt: { receiptRef: string }
    }

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      post: {
        tipRecipientReadiness: {
          blockerRef: null,
          caveatRefs: [
            'caveat.public.forum_tip_recipient.claim_required',
            'caveat.public.forum_tip_recipient.daemon_reachability_required',
            'policy.public.forum_tip_recipient.agent_claimed',
            'policy.public.forum_tip_recipient.self_custody',
          ],
          providerClass: 'mdk_agent_wallet',
          readinessRefs: ['readiness.public.forum_tip_recipient.receive_ready'],
          state: 'ready',
          tippingAvailable: true,
        },
      },
    })
    expect(JSON.stringify(body)).not.toContain(
      'wallet.public.forum_tip_recipient.route_test',
    )
    expect(JSON.stringify(body)).not.toContain(
      'receive_capability.public.forum_tip_recipient.route_test',
    )
    expect(JSON.stringify(body)).not.toContain(
      'approval.public.forum_tip_recipient.route_test',
    )
  })

  test('admits ready Forum tip recipient wallet refs through moderator path', async () => {
    const store = new ForumRouteStore()
    const postId = '66666666-6666-4666-8666-666666666666'
    const admission = await route(
      store,
      '/api/forum/tip-recipient-wallets/admissions',
      {
        body: {
          actorRef: 'actor.route-test',
          bolt12Offer:
            'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
          caveatRefs: ['caveat.public.forum_tip_recipient.claim_required'],
          claimPolicyRefs: ['policy.public.forum_tip_recipient.agent_claimed'],
          custodyPolicyRefs: ['policy.public.forum_tip_recipient.self_custody'],
          payoutTargetApprovalRef: 'approval.public.nexus_pylon.route_test',
          providerClass: 'mdk_agent_wallet',
          readinessRefs: [
            'readiness.public.pylon.wallet_ready',
            'readiness.public.nexus.payout_admitted',
          ],
          receiveCapabilityRef: 'receive_capability.public.pylon.route_test',
          sourceRef: 'source.public.pylon_api.wallet_readiness.route_test',
          state: 'ready',
          walletRef: 'wallet.public.pylon.route_test',
        },
        headers: {
          'idempotency-key': 'forum-tip-recipient-admission-ready',
        },
        method: 'POST',
        moderator: 'admin',
      },
    )
    const detail = await route(store, `/api/forum/posts/${postId}`)
    const preview = await route(store, `/api/forum/posts/${postId}/rewards`, {
      body: {
        requestBodyDigest: 'sha256:forum-reward-after-admission',
        spendCap: { amount: 100, asset: 'sats' },
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'forum-paid-reward-preview-after-admission',
      },
      method: 'POST',
    })
    const admissionBody = await admission.json()
    const detailBody = (await detail.json()) as Readonly<{
      post: Readonly<{
        author?: unknown
      }>
    }>
    const previewBody = await preview.json()

    expect(admission.status).toBe(201)
    expect(admissionBody).toMatchObject({
      moderatorActorRef: 'operator:github:moderator',
      tipRecipientReadiness: {
        actorRef: 'actor.route-test',
        directPayment: {
          bolt12Offer:
            'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
          kind: 'bolt12_offer',
          settlementAuthority: 'recipient_wallet_direct',
        },
        providerClass: 'mdk_agent_wallet',
        state: 'ready',
        tippingAvailable: true,
      },
    })
    expect(detail.status).toBe(200)
    expect(detailBody).toMatchObject({
      post: {
        tipRecipientReadiness: {
          directPayment: {
            kind: 'bolt12_offer',
            settlementAuthority: 'recipient_wallet_direct',
          },
          providerClass: 'mdk_agent_wallet',
          state: 'ready',
          tippingAvailable: true,
        },
      },
    })
    expect(JSON.stringify(detailBody.post.author ?? {})).not.toContain('wallet')
    expect(JSON.stringify(detailBody.post.author ?? {})).not.toContain(
      'receive_capability',
    )
    expect(preview.status).toBe(200)
    expect(previewBody).toMatchObject({
      challenge: null,
      paymentRequired: false,
      writeDenial: {
        denialKind: 'payment_required',
        denialRef: 'blocker.public.forum_tip.bolt12_direct_required',
        payable: false,
      },
    })
    expect(store.challenges).toHaveLength(0)
    expect(JSON.stringify(detailBody)).not.toContain('wallet.public.pylon')
    expect(JSON.stringify(detailBody)).not.toContain(
      'receive_capability.public.pylon',
    )
  })

  test('lets an authenticated agent self-claim ready tip recipient wallet refs', async () => {
    const store = new ForumRouteStore()
    const response = await route(
      store,
      '/api/forum/tip-recipient-wallets/claims',
      {
        body: {
          caveatRefs: ['caveat.public.forum_tip_recipient.claim_doc_pending'],
          bolt12Offer:
            'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
          claimPolicyRefs: ['policy.public.forum_tip_recipient.claimed_by_cli'],
          custodyPolicyRefs: ['policy.public.forum_tip_recipient.self_custody'],
          providerClass: 'mdk_agent_wallet',
          readinessRefs: [
            'readiness.public.mdk_agent.daemon_running',
            'readiness.public.mdk_agent.receive_ready',
            'readiness.public.mdk_agent.setup_present',
          ],
          receiveCapabilityRef:
            'receive_capability.public.mdk_agent_wallet.route_test',
          sourceRef: 'source.public.forum_tip_recipient.claim_route_test',
          walletRef: 'wallet.public.mdk_agent_wallet.route_test',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-tip-recipient-claim-ready',
        },
        method: 'POST',
      },
    )
    const body = (await response.json()) as Readonly<{
      tipRecipientReadiness: Readonly<{
        caveatRefs: ReadonlyArray<string>
      }>
    }>

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      tipRecipientReadiness: {
        actorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
        blockerRef: null,
        directPayment: {
          bolt12Offer:
            'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
          kind: 'bolt12_offer',
          settlementAuthority: 'recipient_wallet_direct',
        },
        providerClass: 'mdk_agent_wallet',
        readinessRefs: [
          'readiness.public.mdk_agent.daemon_running',
          'readiness.public.mdk_agent.receive_ready',
          'readiness.public.mdk_agent.setup_present',
        ],
        state: 'ready',
        tippingAvailable: true,
      },
    })
    expect(body.tipRecipientReadiness.caveatRefs).toEqual(
      expect.arrayContaining([
        'caveat.public.forum_tip_recipient.creator_settlement_pending',
        'caveat.public.forum_tip_recipient.claim_doc_pending',
        'policy.public.forum_tip_recipient.agent_self_claimed',
        'policy.public.forum_tip_recipient.claimed_by_cli',
        'policy.public.forum_tip_recipient.self_custody',
        'policy.public.forum_tip_recipient.self_custody_mdk_agent_wallet',
        'caveat.public.forum_tip_recipient.payout_target_unapproved',
      ]),
    )
    expect(store.tipRecipientWallets).toHaveLength(1)
    expect(store.tipRecipientWallets[0]).toMatchObject({
      actor_ref: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      bolt12_offer:
        'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
      state: 'ready',
    })
    expect(JSON.stringify(body)).not.toContain(
      'wallet.public.mdk_agent_wallet.route_test',
    )
    expect(JSON.stringify(body)).not.toContain(
      'receive_capability.public.mdk_agent_wallet.route_test',
    )
  })

  test('ignores body actor spoofing in self-claim tip wallet requests', async () => {
    const store = new ForumRouteStore()
    const response = await route(
      store,
      '/api/forum/tip-recipient-wallets/claims',
      {
        body: {
          actorRef: 'actor.spoofed-route-test',
          bolt12Offer:
            'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
          readinessRefs: ['readiness.public.mdk_agent_wallet.receive_ready'],
          receiveCapabilityRef:
            'receive_capability.public.mdk_agent_wallet.route_test',
          walletRef: 'wallet.public.mdk_agent_wallet.route_test',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-tip-recipient-claim-spoof',
        },
        method: 'POST',
      },
    )

    expect(response.status).toBe(201)
    await expect(response.json()).resolves.toMatchObject({
      tipRecipientReadiness: {
        actorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
        directPayment: {
          kind: 'bolt12_offer',
          settlementAuthority: 'recipient_wallet_direct',
        },
        state: 'ready',
        tippingAvailable: true,
      },
    })
    expect(store.tipRecipientWallets).toHaveLength(1)
    expect(store.tipRecipientWallets[0]?.actor_ref).toBe(
      'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
    )
  })

  test('requires agent bearer auth for self-claiming tip recipient wallets', async () => {
    const store = new ForumRouteStore()
    const response = await route(
      store,
      '/api/forum/tip-recipient-wallets/claims',
      {
        body: {
          readinessRefs: ['readiness.public.mdk_agent_wallet.receive_ready'],
          receiveCapabilityRef:
            'receive_capability.public.mdk_agent_wallet.route_test',
          walletRef: 'wallet.public.mdk_agent_wallet.route_test',
        },
        headers: {
          'idempotency-key': 'forum-tip-recipient-claim-no-agent',
        },
        method: 'POST',
      },
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toMatchObject({
      error: 'unauthorized',
    })
    expect(store.tipRecipientWallets).toHaveLength(0)
  })

  test('rejects unsafe raw wallet material in self-claim refs', async () => {
    const store = new ForumRouteStore()
    const response = await route(
      store,
      '/api/forum/tip-recipient-wallets/claims',
      {
        body: {
          readinessRefs: ['payment_hash=abc123'],
          receiveCapabilityRef:
            'receive_capability.public.mdk_agent_wallet.route_test',
          walletRef: '/Users/private/.mdk-wallet/config.json',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-tip-recipient-claim-unsafe',
        },
        method: 'POST',
      },
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toMatchObject({
      error: 'bad_request',
    })
    expect(store.tipRecipientWallets).toHaveLength(0)
  })

  test('blocked tip recipient admission immediately prevents reward challenge issuance', async () => {
    const store = new ForumRouteStore()
    const postId = '66666666-6666-4666-8666-666666666666'

    store.tipRecipientWallets.push(readyTipRecipientWalletRow())

    const admission = await route(
      store,
      '/api/forum/tip-recipient-wallets/admissions',
      {
        body: {
          actorRef: 'actor.route-test',
          caveatRefs: ['caveat.public.forum_tip_recipient.policy_blocked'],
          claimPolicyRefs: ['policy.public.forum_tip_recipient.agent_claimed'],
          custodyPolicyRefs: [
            'policy.public.forum_tip_recipient.external_custody_review',
          ],
          providerClass: 'external_lightning',
          readinessRefs: [],
          receiveCapabilityRef:
            'receive_capability.public.nexus_policy.route_test',
          sourceRef: 'source.public.nexus.policy.route_test_blocked',
          state: 'blocked',
          walletRef: 'wallet.public.nexus_policy.route_test',
        },
        headers: {
          'idempotency-key': 'forum-tip-recipient-admission-blocked',
        },
        method: 'POST',
        moderator: 'admin',
      },
    )
    const detail = await route(store, `/api/forum/posts/${postId}`)
    const preview = await route(store, `/api/forum/posts/${postId}/rewards`, {
      body: {
        requestBodyDigest: 'sha256:forum-reward-after-blocked-admission',
        spendCap: { amount: 100, asset: 'sats' },
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'forum-paid-reward-preview-after-blocked-admission',
      },
      method: 'POST',
    })

    expect(admission.status).toBe(201)
    await expect(admission.json()).resolves.toMatchObject({
      tipRecipientReadiness: {
        blockerRef: 'blocker.public.forum_tip_recipient.actor_blocked',
        providerClass: 'external_lightning',
        state: 'blocked',
        tippingAvailable: false,
      },
    })
    expect(detail.status).toBe(200)
    await expect(detail.json()).resolves.toMatchObject({
      post: {
        tipRecipientReadiness: {
          blockerRef: 'blocker.public.forum_tip_recipient.actor_blocked',
          state: 'blocked',
          tippingAvailable: false,
        },
      },
    })
    expect(preview.status).toBe(200)
    await expect(preview.json()).resolves.toMatchObject({
      challenge: null,
      paymentRequired: false,
      writeDenial: {
        denialKind: 'recipient_not_ready',
        denialRef: 'blocker.public.forum_tip_recipient.actor_blocked',
      },
    })
    expect(store.challenges).toHaveLength(0)
  })

  test('rejects unsafe raw wallet material in tip recipient admissions', async () => {
    const store = new ForumRouteStore()
    const response = await route(
      store,
      '/api/forum/tip-recipient-wallets/admissions',
      {
        body: {
          actorRef: 'actor.route-test',
          providerClass: 'mdk_agent_wallet',
          readinessRefs: ['readiness.public.pylon.wallet_ready'],
          receiveCapabilityRef: 'receive_capability.public.pylon.route_test',
          sourceRef: 'source.public.pylon_api.wallet_readiness.route_test',
          state: 'ready',
          walletRef: 'lnbc10n1rawinvoice',
        },
        headers: {
          'idempotency-key': 'forum-tip-recipient-admission-unsafe',
        },
        method: 'POST',
        moderator: 'admin',
      },
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toMatchObject({
      error: 'bad_request',
    })
    expect(store.tipRecipientWallets).toHaveLength(0)
  })

  test('lists public-safe posts with listed default and authenticated unlisted discovery', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    store.topics.push({
      actor_json: actorJson,
      archived_at: null,
      created_at: '2026-06-05T20:05:00.000Z',
      first_post_id: '99999999-9999-4999-8999-999999999999',
      forum_id: '77777777-1111-4111-8111-777777777777',
      id: '88888888-8888-4888-8888-888888888888',
      idempotency_key: 'void-topic',
      latest_post_id: '99999999-9999-4999-8999-999999999999',
      pin_state: 'normal',
      post_count: 1,
      public_projection_json: projectionJson,
      score_ref: null,
      slug: 'void-topic',
      state: 'open',
      title: 'Void Topic',
      updated_at: '2026-06-05T20:05:00.000Z',
    })
    store.posts.push({
      actor_json: actorJson,
      archived_at: null,
      body_text: 'Void route-test body.',
      content_ref: 'content.forum.route_test.void',
      created_at: '2026-06-05T20:05:00.000Z',
      forum_id: '77777777-1111-4111-8111-777777777777',
      id: '99999999-9999-4999-8999-999999999999',
      idempotency_key: 'void-post',
      parent_post_id: null,
      post_number: 1,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '88888888-8888-4888-8888-888888888888',
      updated_at: '2026-06-05T20:05:00.000Z',
    })

    const listed = await route(store, '/api/forum/posts')
    const exactVoid = await route(store, '/api/forum/posts?forumRef=void')
    const unauthorizedUnlisted = await route(
      store,
      '/api/forum/posts?include=unlisted',
    )
    const authedUnlisted = await route(
      store,
      '/api/forum/posts?include=unlisted',
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
        },
      },
    )

    expect(listed.status).toBe(200)
    const listedBody = (await listed.json()) as Readonly<{
      forums: ReadonlyArray<Readonly<{ slug: string }>>
      includeUnlisted: boolean
      pagination: Readonly<{ hasMore: boolean; limit: number }>
      posts: ReadonlyArray<
        Readonly<{
          bodyText: string
          capabilities: Readonly<{ canTip: boolean }>
          postId: string
          tipRecipientReadiness: Readonly<{
            blockerRef: string | null
            providerClass: string | null
            state: string
            tippingAvailable: boolean
          }>
        }>
      >
      topics: ReadonlyArray<Readonly<{ title: string }>>
    }>
    expect(listedBody).toMatchObject({
      includeUnlisted: false,
      pagination: { hasMore: false, limit: 50 },
    })
    expect(listedBody.forums.map(forum => forum.slug)).toEqual(
      expect.arrayContaining(['artanis', 'site-builder-help']),
    )
    expect(listedBody.posts).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ bodyText: 'Seed route-test body.' }),
      ]),
    )
    expect(
      listedBody.posts.find(
        post => post.postId === '66666666-6666-4666-8666-666666666666',
      ),
    ).toMatchObject({
      capabilities: { canTip: true },
      tipRecipientReadiness: {
        blockerRef: null,
        providerClass: 'mdk_agent_wallet',
        state: 'ready',
        tippingAvailable: true,
      },
    })
    expect(listedBody.posts).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({ bodyText: 'Void route-test body.' }),
      ]),
    )
    expect(listedBody.topics).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ title: 'First Topic' }),
      ]),
    )
    await expect(exactVoid.json()).resolves.toMatchObject({
      forums: [{ slug: 'void' }],
      posts: [{ bodyText: 'Void route-test body.' }],
      topics: [{ title: 'Void Topic' }],
    })
    expect(unauthorizedUnlisted.status).toBe(401)
    const authedUnlistedBody = (await authedUnlisted.json()) as Readonly<{
      includeUnlisted: boolean
      posts: ReadonlyArray<Readonly<{ bodyText: string }>>
    }>
    expect(authedUnlistedBody.includeUnlisted).toBe(true)
    expect(authedUnlistedBody.posts).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ bodyText: 'Void route-test body.' }),
        expect.objectContaining({ bodyText: 'Seed route-test body.' }),
      ]),
    )
  })

  test('paginates aggregate post list with an opaque cursor and rejects bad limits', async () => {
    const store = new ForumRouteStore()
    store.posts.push({
      actor_json: actorJson,
      archived_at: null,
      body_text: 'Newer listed body.',
      content_ref: 'content.forum.route_test.newer',
      created_at: '2026-06-05T20:10:00.000Z',
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: '77777777-7777-4777-8777-777777777777',
      idempotency_key: 'newer-post',
      parent_post_id: null,
      post_number: 2,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '55555555-5555-4555-8555-555555555555',
      updated_at: '2026-06-05T20:10:00.000Z',
    })

    const firstPageResponse = await route(
      store,
      '/api/forum/posts?forumRef=site-builder-help&limit=1',
    )
    const firstPage = (await firstPageResponse.json()) as Readonly<{
      pagination: Readonly<{ hasMore: boolean; nextCursor: string | null }>
      posts: ReadonlyArray<Readonly<{ bodyText: string }>>
    }>
    const secondPageResponse = await route(
      store,
      `/api/forum/posts?forumRef=site-builder-help&limit=1&cursor=${encodeURIComponent(
        firstPage.pagination.nextCursor ?? '',
      )}`,
    )
    const highLimit = await route(store, '/api/forum/posts?limit=101')
    const badCursor = await route(store, '/api/forum/posts?cursor=not-a-cursor')

    expect(firstPageResponse.status).toBe(200)
    expect(firstPage.posts).toStrictEqual([
      expect.objectContaining({ bodyText: 'Newer listed body.' }),
    ])
    expect(firstPage.pagination.hasMore).toBe(true)
    expect(firstPage.pagination.nextCursor).toEqual(expect.any(String))
    await expect(secondPageResponse.json()).resolves.toMatchObject({
      pagination: { cursor: firstPage.pagination.nextCursor, limit: 1 },
      posts: [{ bodyText: 'Seed route-test body.' }],
    })
    expect(highLimit.status).toBe(400)
    expect(badCursor.status).toBe(400)
  })

  test('reads public-safe agent profiles without credential or email material', async () => {
    const store = new ForumRouteStore()
    const visibleSlugAgentId = 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'
    const visibleSlugActorJson = JSON.stringify({
      actorId: visibleSlugAgentId,
      actorRef: `agent:${visibleSlugAgentId}`,
      displayName: 'Visible Slug',
      groupRefs: ['agents'],
      isAgent: true,
      slug: 'visible-slug',
    })
    store.agentProfiles.push({
      avatar_url: null,
      created_at: '2026-06-05T21:00:00.000Z',
      display_name: 'Visible Slug Agent',
      slug: 'visible-slug-agent',
      updated_at: '2026-06-05T21:00:00.000Z',
      user_id: visibleSlugAgentId,
    })
    store.posts.push({
      actor_json: visibleSlugActorJson,
      archived_at: null,
      body_text: 'Visible slug agent introduction.',
      content_ref: 'content.forum.visible_slug_agent.introduction',
      created_at: '2026-06-05T21:00:00.000Z',
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: 'bbbbbbbb-1111-4111-8111-bbbbbbbbbbbb',
      idempotency_key: 'seed-visible-slug-agent-post',
      parent_post_id: null,
      post_number: 2,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '55555555-5555-4555-8555-555555555555',
      updated_at: '2026-06-05T21:00:00.000Z',
    })
    store.topics.push({
      actor_json: visibleSlugActorJson,
      archived_at: null,
      created_at: '2026-06-05T21:05:00.000Z',
      first_post_id: 'bbbbbbbb-2222-4111-8111-bbbbbbbbbbbb',
      forum_id: '77777777-1111-4111-8111-777777777777',
      id: 'bbbbbbbb-2222-4111-8111-bbbbbbbbbbbb',
      idempotency_key: 'seed-visible-slug-agent-void-topic',
      latest_post_id: 'bbbbbbbb-2222-4111-8111-bbbbbbbbbbbb',
      pin_state: 'normal',
      post_count: 1,
      public_projection_json: projectionJson,
      score_ref: null,
      slug: 'visible-slug-void-topic',
      state: 'open',
      title: 'Visible Slug Void Topic',
      updated_at: '2026-06-05T21:05:00.000Z',
    })
    store.posts.push(
      {
        actor_json: visibleSlugActorJson,
        archived_at: null,
        body_text: 'Visible slug hidden moderation row.',
        content_ref: 'content.forum.visible_slug_agent.hidden',
        created_at: '2026-06-05T21:06:00.000Z',
        forum_id: '33333333-3333-4333-8333-333333333333',
        id: 'bbbbbbbb-3333-4111-8111-bbbbbbbbbbbb',
        idempotency_key: 'seed-visible-slug-agent-hidden',
        parent_post_id: null,
        post_number: 3,
        public_projection_json: projectionJson,
        quote_post_id: null,
        receipt_refs_json: '["receipt.public.hidden.should_not_leak"]',
        revision_ref: null,
        state: 'hidden',
        topic_id: '55555555-5555-4555-8555-555555555555',
        updated_at: '2026-06-05T21:06:00.000Z',
      },
      {
        actor_json: visibleSlugActorJson,
        archived_at: null,
        body_text: 'Visible slug held moderation row.',
        content_ref: 'content.forum.visible_slug_agent.held',
        created_at: '2026-06-05T21:07:00.000Z',
        forum_id: '33333333-3333-4333-8333-333333333333',
        id: 'bbbbbbbb-4444-4111-8111-bbbbbbbbbbbb',
        idempotency_key: 'seed-visible-slug-agent-held',
        parent_post_id: null,
        post_number: 4,
        public_projection_json: projectionJson,
        quote_post_id: null,
        receipt_refs_json: '["receipt.public.held.should_not_leak"]',
        revision_ref: null,
        state: 'held_for_review',
        topic_id: '55555555-5555-4555-8555-555555555555',
        updated_at: '2026-06-05T21:07:00.000Z',
      },
      {
        actor_json: visibleSlugActorJson,
        archived_at: null,
        body_text: 'Visible slug tombstoned row.',
        content_ref: 'content.forum.visible_slug_agent.tombstoned',
        created_at: '2026-06-05T21:08:00.000Z',
        forum_id: '33333333-3333-4333-8333-333333333333',
        id: 'bbbbbbbb-5555-4111-8111-bbbbbbbbbbbb',
        idempotency_key: 'seed-visible-slug-agent-tombstoned',
        parent_post_id: null,
        post_number: 5,
        public_projection_json: projectionJson,
        quote_post_id: null,
        receipt_refs_json: '["receipt.public.tombstoned.should_not_leak"]',
        revision_ref: null,
        state: 'tombstoned',
        topic_id: '55555555-5555-4555-8555-555555555555',
        updated_at: '2026-06-05T21:08:00.000Z',
      },
      {
        actor_json: visibleSlugActorJson,
        archived_at: null,
        body_text: 'Visible slug unlisted row.',
        content_ref: 'content.forum.visible_slug_agent.unlisted',
        created_at: '2026-06-05T21:09:00.000Z',
        forum_id: '77777777-1111-4111-8111-777777777777',
        id: 'bbbbbbbb-6666-4111-8111-bbbbbbbbbbbb',
        idempotency_key: 'seed-visible-slug-agent-unlisted',
        parent_post_id: null,
        post_number: 1,
        public_projection_json: projectionJson,
        quote_post_id: null,
        receipt_refs_json: '["receipt.public.unlisted.should_not_leak"]',
        revision_ref: null,
        state: 'visible',
        topic_id: 'bbbbbbbb-2222-4111-8111-bbbbbbbbbbbb',
        updated_at: '2026-06-05T21:09:00.000Z',
      },
    )
    const profileResponse = await route(
      store,
      '/api/agents/profiles/route-test-agent',
    )
    const visibleSlugProfileResponse = await route(
      store,
      '/api/agents/profiles/visible-slug',
    )
    const agentProfileRefResponse = await route(
      store,
      `/api/agents/profiles/${encodeURIComponent(`agent_profile:${visibleSlugAgentId}`)}`,
    )
    const browserProfileResponse = await route(
      store,
      `/forum/u/${visibleSlugAgentId}/visible-slug-agent`,
    )
    const redirectResponse = await route(store, '/agents/visible-slug')
    const snapshotResponse = await route(
      store,
      `/api/forum/actors/${encodeURIComponent('actor.route-test')}/profile`,
    )
    const profile = await profileResponse.json()
    const visibleSlugProfile = await visibleSlugProfileResponse.json()
    const agentProfileRef = await agentProfileRefResponse.json()
    const browserProfile = await browserProfileResponse.text()
    const snapshot = await snapshotResponse.json()

    expect(profileResponse.status).toBe(200)
    expect(profile).toMatchObject({
      profile: {
        actor: {
          actorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
          displayName: 'Route Test Agent',
          isAgent: true,
          slug: 'route-test-agent',
        },
        source: 'agent_profile',
        verificationState: 'registered_agent',
      },
    })
    expect(visibleSlugProfileResponse.status).toBe(200)
    expect(visibleSlugProfile).toMatchObject({
      profile: {
        actor: {
          actorRef: `agent:${visibleSlugAgentId}`,
          displayName: 'Visible Slug Agent',
          slug: 'visible-slug-agent',
        },
        ownerHandoff: {
          agentTokenStatus: 'created',
          claimEndpoint: 'https://openagents.com/api/agents/claims',
          humanLoginStatus: 'owner_claim_required',
          ownerLoginTemplate:
            'https://openagents.com/login/github?returnTo=/agents/claims/{claimId}',
        },
        publicUrl: `https://openagents.com/forum/u/${visibleSlugAgentId}/visible-slug-agent`,
        source: 'agent_profile',
      },
    })
    expect(visibleSlugProfile).toMatchObject({
      profile: {
        activity: [
          expect.objectContaining({
            href: 'https://openagents.com/forum/t/55555555-5555-4555-8555-555555555555#post-bbbbbbbb-1111-4111-8111-bbbbbbbbbbbb',
            kind: 'post',
            postId: 'bbbbbbbb-1111-4111-8111-bbbbbbbbbbbb',
            receiptRefs: [],
            title: 'First Topic',
          }),
        ],
      },
    })
    expect(agentProfileRefResponse.status).toBe(200)
    expect(agentProfileRef).toMatchObject({
      profile: {
        actor: {
          actorRef: `agent:${visibleSlugAgentId}`,
          slug: 'visible-slug-agent',
        },
        activity: [
          expect.objectContaining({
            postId: 'bbbbbbbb-1111-4111-8111-bbbbbbbbbbbb',
            title: 'First Topic',
          }),
        ],
      },
    })
    expect(browserProfileResponse.status).toBe(200)
    expect(browserProfileResponse.headers.get('content-type')).toContain(
      'text/html',
    )
    expect(browserProfile).toContain('data-agent-profile-page')
    expect(browserProfile).toContain('Visible Slug Agent')
    expect(browserProfile).toContain('Public activity')
    expect(browserProfile).toContain('First Topic')
    expect(browserProfile).toContain('Tips')
    expect(browserProfile).toContain('Not enabled - no tip wallet claimed yet')
    expect(browserProfile).toContain('No settled tips yet')
    expect(browserProfile).toContain(
      'https://openagents.com/login/github?returnTo=/agents/claims/CLAIM_ID',
    )
    expect(redirectResponse.status).toBe(302)
    expect(redirectResponse.headers.get('location')).toBe(
      `https://openagents.com/forum/u/${visibleSlugAgentId}/visible-slug-agent`,
    )
    expect(snapshotResponse.status).toBe(200)
    expect(snapshot).toMatchObject({
      profile: {
        actor: { actorRef: 'actor.route-test', isAgent: true },
        source: 'forum_actor_snapshot',
      },
    })
    expect(JSON.stringify(profile)).not.toContain('agent@example.com')
    expect(JSON.stringify(profile)).not.toContain('oa_agent')
    expect(JSON.stringify(visibleSlugProfile)).not.toContain(
      'should_not_leak',
    )
    expect(browserProfile).not.toContain('should_not_leak')
    expect(browserProfile).not.toContain('Visible Slug Void Topic')
  })

  test('refreshes public agent profiles from approved owner claims', async () => {
    const store = new ForumRouteStore()
    const agentUserId = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'
    const beforeResponse = await route(
      store,
      '/api/agents/profiles/route-test-agent',
    )
    const before = await beforeResponse.json()

    store.agentOwnerClaims.push({
      agent_user_id: agentUserId,
      decided_at: '2026-06-10T21:27:56.197Z',
      id: 'agent_claim_45535152-f195-4b01-95fa-0c1b9bf1f6ff',
      owner_user_id: 'github:17035300',
      receipt_ref:
        'agent_claim_receipt_agent_claim_45535152-f195-4b01-95fa-0c1b9bf1f6ff',
      status: 'approved',
      updated_at: '2026-06-10T21:27:56.197Z',
    })

    const afterResponse = await route(
      store,
      '/api/agents/profiles/route-test-agent',
    )
    const browserResponse = await route(
      store,
      `/forum/u/${agentUserId}/route-test-agent`,
    )
    const after = await afterResponse.json()
    const browserProfile = await browserResponse.text()

    expect(beforeResponse.status).toBe(200)
    expect(before).toMatchObject({
      profile: {
        ownerHandoff: {
          humanLoginStatus: 'owner_claim_required',
          ownerUserRef: null,
        },
        updatedAt: '2026-06-05T20:00:00.000Z',
        verificationState: 'registered_agent',
      },
    })
    expect(afterResponse.status).toBe(200)
    expect(after).toMatchObject({
      profile: {
        ownerHandoff: {
          claimReceiptRefs: [
            'agent_claim_receipt_agent_claim_45535152-f195-4b01-95fa-0c1b9bf1f6ff',
          ],
          claimRef: 'agent_claim_45535152-f195-4b01-95fa-0c1b9bf1f6ff',
          humanLoginStatus: 'owner_claim_approved',
          ownerUserRef: 'owner:github:17035300',
        },
        publicProjection: {
          safeReceiptRefs: [
            'agent_claim_receipt_agent_claim_45535152-f195-4b01-95fa-0c1b9bf1f6ff',
          ],
          trustTier: 'verified',
        },
        updatedAt: '2026-06-10T21:27:56.197Z',
        verificationState: 'owner_claimed_agent',
      },
    })
    expect(browserResponse.status).toBe(200)
    expect(browserProfile).toContain('owner_claim_approved')
    expect(browserProfile).toContain('owner:github:17035300')
    expect(browserProfile).toContain(
      'agent_claim_receipt_agent_claim_45535152-f195-4b01-95fa-0c1b9bf1f6ff',
    )
    expect(JSON.stringify(after)).not.toContain('agent@example.com')
    expect(JSON.stringify(after)).not.toContain('oa_agent')
    expect(browserProfile).not.toContain('CLAIM_ID')
    expect(browserProfile).not.toContain('owner_claim_required')
  })

  // Epic #4751 instances 1-2 (#4744): the profile projection composes
  // the verified X-proof challenge live and declares its staleness
  // contract, so neither the owner-claim write nor the X-verification
  // write can be lost by a frozen public trust surface.
  test('reflects verified X proofs on agent profiles and declares projection staleness', async () => {
    const store = new ForumRouteStore()
    const agentUserId = 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'

    store.agentOwnerClaims.push({
      agent_user_id: agentUserId,
      decided_at: '2026-06-10T21:27:56.197Z',
      id: 'agent_claim_45535152-f195-4b01-95fa-0c1b9bf1f6ff',
      owner_user_id: 'github:17035300',
      receipt_ref:
        'agent_claim_receipt_agent_claim_45535152-f195-4b01-95fa-0c1b9bf1f6ff',
      status: 'approved',
      updated_at: '2026-06-10T21:27:56.197Z',
    })

    const claimedResponse = await route(
      store,
      '/api/agents/profiles/route-test-agent',
    )
    const claimed = (await claimedResponse.json()) as {
      generatedAt?: string
      profile: { verificationState: string }
      staleness?: Record<string, unknown>
    }

    store.agentOwnerXChallenges.push({
      agent_user_id: agentUserId,
      id: 'x_claim_challenge_5f0a8e3c',
      receipt_ref: 'x_claim_receipt_x_claim_challenge_5f0a8e3c',
      state: 'verified',
      tweet_ref: 'tweet:1932575113341271138',
      updated_at: '2026-06-11T01:12:00.000Z',
      verified_at: '2026-06-11T01:12:00.000Z',
    })

    const verifiedResponse = await route(
      store,
      '/api/agents/profiles/route-test-agent',
    )
    const verified = await verifiedResponse.json()

    expect(claimedResponse.status).toBe(200)
    expect(claimed.profile.verificationState).toBe('owner_claimed_agent')
    expect(typeof claimed.generatedAt).toBe('string')
    expect(claimed.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
      rebuildsOn: expect.arrayContaining([
        'agent_owner_claim_approved',
        'agent_owner_x_claim_verified',
      ]),
    })
    expect(verifiedResponse.status).toBe(200)
    expect(verified).toMatchObject({
      profile: {
        publicProjection: {
          safeReceiptRefs: [
            'agent_claim_receipt_agent_claim_45535152-f195-4b01-95fa-0c1b9bf1f6ff',
            'x_claim_receipt_x_claim_challenge_5f0a8e3c',
          ],
          trustTier: 'verified',
        },
        updatedAt: '2026-06-11T01:12:00.000Z',
        verificationState: 'x_verified_agent',
      },
    })
    // The verified X identity surfaces as refs only — no handle, token,
    // or raw tweet URL leaves the ledger through this projection.
    expect(JSON.stringify(verified)).not.toContain('oauth')
    expect(JSON.stringify(verified)).not.toContain('x.com/')
  })

  test('creates idempotent watches, bookmarks, and follows for authorized agents', async () => {
    const store = new ForumRouteStore()
    const topicId = '55555555-5555-4555-8555-555555555555'
    const postId = '66666666-6666-4666-8666-666666666666'
    const watchTopic = await route(
      store,
      `/api/forum/topics/${topicId}/watches`,
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'watch-topic-1',
        },
        method: 'POST',
      },
    )
    const watchTopicRetry = await route(
      store,
      `/api/forum/topics/${topicId}/watches`,
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'watch-topic-1',
        },
        method: 'POST',
      },
    )
    const bookmarkPost = await route(
      store,
      `/api/forum/posts/${postId}/bookmarks`,
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'bookmark-post-1',
        },
        method: 'POST',
      },
    )
    const follow = await route(
      store,
      `/api/forum/actors/${encodeURIComponent('actor.route-test')}/follows`,
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'follow-actor-1',
        },
        method: 'POST',
      },
    )

    expect(watchTopic.status).toBe(201)
    expect(watchTopicRetry.status).toBe(200)
    expect(bookmarkPost.status).toBe(201)
    expect(follow.status).toBe(201)
    await expect(watchTopic.json()).resolves.toMatchObject({
      action: 'watch',
      actorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      idempotent: false,
      target: { topicId },
    })
    await expect(watchTopicRetry.json()).resolves.toMatchObject({
      action: 'watch',
      idempotent: true,
      target: { topicId },
    })
    await expect(bookmarkPost.json()).resolves.toMatchObject({
      action: 'bookmark',
      target: { postId, topicId },
    })
    await expect(follow.json()).resolves.toMatchObject({
      action: 'follow',
      target: { actorRef: 'actor.route-test' },
    })
    expect(store.watches).toHaveLength(1)
    expect(store.bookmarks).toHaveLength(1)
    expect(store.follows).toHaveLength(1)
  })

  test('returns scoped redacted notifications for watched topics, follows, mentions, and receipts', async () => {
    const store = new ForumRouteStore()
    store.posts[0] = {
      ...store.posts[0]!,
      body_text: 'Seed route-test body mentioning @route-test-agent.',
    }
    store.watches.push({
      actor_ref: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      archived_at: null,
      created_at: '2026-06-05T20:05:00.000Z',
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: 'watch-topic-existing',
      idempotency_key: 'watch-topic-existing',
      topic_id: '55555555-5555-4555-8555-555555555555',
      watch_kind: 'topic',
    })
    store.follows.push({
      actor_ref: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      archived_at: null,
      created_at: '2026-06-05T20:05:00.000Z',
      id: 'follow-existing',
      idempotency_key: 'follow-existing',
      target_actor_ref: 'actor.route-test',
    })
    store.receipts.push({
      action_kind: 'post_reward',
      amount_asset: 'sats',
      amount_value: 100,
      archived_at: null,
      created_at: '2026-06-05T20:06:00.000Z',
      id: 'receipt-notification-1',
      public_projection_json: projectionJson,
      receipt_ref: 'receipt.forum.notification.1',
      recipient_actor_ref: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      redacted_payment_ref: 'mdk_redacted_receipt_ref',
      target_forum_id: '33333333-3333-4333-8333-333333333333',
      target_post_id: '66666666-6666-4666-8666-666666666666',
      target_topic_id: '55555555-5555-4555-8555-555555555555',
    })

    const response = await route(store, '/api/agents/notifications', {
      headers: { authorization: 'Bearer oa_agent_route_test' },
    })
    const body = JSON.parse(await response.text()) as Readonly<{
      notifications: ReadonlyArray<
        Readonly<{
          id: string
          kind: string
          publicUrl: string
          readState: string
        }>
      >
    }>
    const notifications = [...body.notifications]
    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      actorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      summary: {
        mentionCount: 1,
        receiptCount: 1,
        totalCount: 4,
        unreadCount: 4,
        watchedTopicReplyCount: 1,
      },
      notifications: expect.arrayContaining([
        expect.objectContaining({ kind: 'watched_topic_reply' }),
        expect.objectContaining({ kind: 'followed_actor_post' }),
        expect.objectContaining({ kind: 'mention' }),
        expect.objectContaining({ kind: 'receipt' }),
      ]),
    })
    expect(JSON.stringify(body)).not.toContain('mdk_redacted_receipt_ref')
    expect(JSON.stringify(body)).not.toContain('agent@example.com')

    const mention = notifications.find(
      notification => notification.kind === 'mention',
    )
    const watchedTopicReply = notifications.find(
      notification => notification.kind === 'watched_topic_reply',
    )
    const followedActorPost = notifications.find(
      notification => notification.kind === 'followed_actor_post',
    )
    const receipt = notifications.find(
      notification => notification.kind === 'receipt',
    )

    expect(mention).toBeDefined()
    expect(watchedTopicReply).toBeDefined()
    expect(followedActorPost).toBeDefined()
    expect(receipt).toBeDefined()
    expect(mention!.publicUrl).toBe(
      'https://openagents.com/forum/t/55555555-5555-4555-8555-555555555555#post-66666666-6666-4666-8666-666666666666',
    )
    expect(watchedTopicReply!.publicUrl).toBe(mention!.publicUrl)
    expect(followedActorPost!.publicUrl).toBe(mention!.publicUrl)

    const markRead = await route(
      store,
      `/api/agents/notifications/${encodeURIComponent(mention!.id)}/read`,
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'notification-read-mention-1',
        },
        method: 'POST',
      },
    )
    const retry = await route(
      store,
      `/api/agents/notifications/${encodeURIComponent(mention!.id)}/read`,
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'notification-read-mention-1',
        },
        method: 'POST',
      },
    )
    const conflict = await route(
      store,
      `/api/agents/notifications/${encodeURIComponent(receipt!.id)}/read`,
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'notification-read-mention-1',
        },
        method: 'POST',
      },
    )
    const readFeed = await route(store, '/api/agents/notifications', {
      headers: { authorization: 'Bearer oa_agent_route_test' },
    })
    const readBody = await readFeed.json()

    expect(markRead.status).toBe(201)
    await expect(markRead.json()).resolves.toMatchObject({
      idempotent: false,
      notificationId: mention!.id,
    })
    expect(retry.status).toBe(200)
    await expect(retry.json()).resolves.toMatchObject({
      idempotent: true,
      notificationId: mention!.id,
    })
    expect(conflict.status).toBe(409)
    await expect(conflict.json()).resolves.toMatchObject({
      error: 'idempotency_key_conflict',
    })
    expect(readBody).toMatchObject({
      summary: { totalCount: 4, unreadCount: 3 },
      notifications: expect.arrayContaining([
        expect.objectContaining({
          id: mention!.id,
          readAt: '2026-06-05T20:00:00.000Z',
          readState: 'read',
        }),
      ]),
    })
  })

  test('filters the notifications array to unread items when unread=true is set', async () => {
    const store = new ForumRouteStore()
    store.posts[0] = {
      ...store.posts[0]!,
      body_text: 'Seed route-test body mentioning @route-test-agent.',
    }
    store.watches.push({
      actor_ref: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      archived_at: null,
      created_at: '2026-06-05T20:05:00.000Z',
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: 'watch-topic-existing',
      idempotency_key: 'watch-topic-existing',
      topic_id: '55555555-5555-4555-8555-555555555555',
      watch_kind: 'topic',
    })

    const initialResponse = await route(store, '/api/agents/notifications', {
      headers: { authorization: 'Bearer oa_agent_route_test' },
    })
    const initialBody = JSON.parse(await initialResponse.text()) as Readonly<{
      notifications: ReadonlyArray<
        Readonly<{ id: string; kind: string; readState: string }>
      >
      summary: Readonly<{ unreadCount: number; totalCount: number }>
    }>
    expect(initialResponse.status).toBe(200)
    const initialNotifications = [...initialBody.notifications]
    const mention = initialNotifications.find(
      notification => notification.kind === 'mention',
    )
    const watchedTopicReply = initialNotifications.find(
      notification => notification.kind === 'watched_topic_reply',
    )
    expect(mention).toBeDefined()
    expect(watchedTopicReply).toBeDefined()
    expect(initialNotifications.every(n => n.readState === 'unread')).toBe(true)

    // Mark exactly one notification (the mention) as read.
    const markRead = await route(
      store,
      `/api/agents/notifications/${encodeURIComponent(mention!.id)}/read`,
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'notification-read-unread-filter-1',
        },
        method: 'POST',
      },
    )
    expect(markRead.status).toBe(201)

    // No param: the broad feed still returns both the read and unread items.
    const broadResponse = await route(store, '/api/agents/notifications', {
      headers: { authorization: 'Bearer oa_agent_route_test' },
    })
    const broadBody = JSON.parse(await broadResponse.text()) as Readonly<{
      notifications: ReadonlyArray<
        Readonly<{ id: string; kind: string; readState: string }>
      >
      summary: Readonly<{ unreadCount: number; totalCount: number }>
    }>
    expect(broadResponse.status).toBe(200)
    const broadNotifications = [...broadBody.notifications]
    expect(broadNotifications).toHaveLength(initialNotifications.length)
    expect(
      broadNotifications.some(
        notification =>
          notification.id === mention!.id && notification.readState === 'read',
      ),
    ).toBe(true)
    expect(
      broadNotifications.some(
        notification => notification.id === watchedTopicReply!.id,
      ),
    ).toBe(true)

    // unread=true: only the unread item(s) come back in the array, but the
    // summary still reports the true server-computed unread count.
    const unreadResponse = await route(
      store,
      '/api/agents/notifications?unread=true',
      { headers: { authorization: 'Bearer oa_agent_route_test' } },
    )
    const unreadBody = JSON.parse(await unreadResponse.text()) as Readonly<{
      notifications: ReadonlyArray<
        Readonly<{ id: string; kind: string; readState: string }>
      >
      summary: Readonly<{ unreadCount: number; totalCount: number }>
    }>
    expect(unreadResponse.status).toBe(200)
    const unreadNotifications = [...unreadBody.notifications]
    expect(
      unreadNotifications.every(
        notification => notification.readState === 'unread',
      ),
    ).toBe(true)
    expect(
      unreadNotifications.some(
        notification => notification.id === mention!.id,
      ),
    ).toBe(false)
    expect(
      unreadNotifications.some(
        notification => notification.id === watchedTopicReply!.id,
      ),
    ).toBe(true)

    // summary.unreadCount is consistent across both views and equals the true
    // server unread count (one item was marked read).
    expect(broadBody.summary.unreadCount).toBe(initialBody.summary.unreadCount - 1)
    expect(unreadBody.summary.unreadCount).toBe(broadBody.summary.unreadCount)
    expect(unreadBody.summary.totalCount).toBe(broadBody.summary.totalCount)
    // The filtered array length matches the true unread count here.
    expect(unreadNotifications).toHaveLength(unreadBody.summary.unreadCount)
  })

  test('denies participation writes and notifications without a bearer token', async () => {
    const store = new ForumRouteStore()
    const watch = await route(
      store,
      '/api/forum/topics/55555555-5555-4555-8555-555555555555/watches',
      {
        headers: { 'idempotency-key': 'watch-topic-no-auth' },
        method: 'POST',
      },
    )
    const notifications = await route(store, '/api/agents/notifications')
    const notificationRead = await route(
      store,
      '/api/agents/notifications/mention%3Apost/read',
      {
        headers: { 'idempotency-key': 'notification-read-no-auth' },
        method: 'POST',
      },
    )

    expect(watch.status).toBe(401)
    expect(notifications.status).toBe(401)
    expect(notificationRead.status).toBe(401)
    expect(store.watches).toHaveLength(0)
  })

  test('blocks the old hosted L402 path for ordinary Forum rewards', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    const postId = '66666666-6666-4666-8666-666666666666'
    const path = `/api/forum/posts/${postId}/rewards`
    const previewResponse = await route(store, path, {
      body: {
        requestBodyDigest: 'sha256:forum-reward-body',
        spendCap: { amount: 100, asset: 'sats' },
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'forum-paid-reward-preview-1',
      },
      method: 'POST',
    })
    const postDetailResponse = await route(
      store,
      `/api/forum/posts/${encodeURIComponent(postId)}`,
    )
    const postDetail = await postDetailResponse.json()
    const leaderboardsResponse = await route(
      store,
      '/api/forum/tip-leaderboards',
    )
    const leaderboards = await leaderboardsResponse.json()

    expect(previewResponse.status).toBe(200)
    expect(previewResponse.headers.get('www-authenticate')).toBeNull()
    await expect(previewResponse.json()).resolves.toStrictEqual({
      challenge: null,
      entitlementRef: null,
      paymentRequired: false,
      writeDenial: {
        actorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
        denialKind: 'payment_required',
        denialRef: 'blocker.public.forum_tip.bolt12_direct_required',
        payable: false,
        requiredPermission: null,
      },
    })
    expect(store.challenges).toHaveLength(0)
    expect(store.receipts).toHaveLength(0)
    expect(store.moneyActions).toHaveLength(0)
    expect(store.paymentEvents).toHaveLength(0)
    expect(postDetailResponse.status).toBe(200)
    expect(postDetail).toMatchObject({
      post: {
        postId,
        tipStats: {
          tipCount: 0,
          totalCreditedSats: 0,
          totalPaidSats: 0,
          totalSettledSats: 0,
        },
      },
    })
    expect(leaderboardsResponse.status).toBe(200)
    expect(leaderboards).toMatchObject({
      creators: [],
      posts: [],
    })
  })

  test('submits and reads a BOLT 12 direct Forum tip without L402', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    const postId = '66666666-6666-4666-8666-666666666666'
    const response = await route(
      store,
      `/api/forum/posts/${encodeURIComponent(postId)}/direct-tips`,
      {
        body: {
          amount: { amount: 15, asset: 'sats' },
          paymentEvidence: {
            externalRef: 'external.payment.redacted.route_direct_tip_1',
            paymentMode: 'live',
            providerRef: 'provider.mdk_agent_wallet.redacted',
            redactedEvidenceRef:
              'evidence.payment.redacted.route_direct_tip_1',
            status: 'confirmed',
          },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-direct-tip-route-1',
        },
        method: 'POST',
      },
    )
    const body = (await response.json()) as {
      attemptId: string
      receipt: { receiptRef: string }
    }

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      amount: { amount: 15, asset: 'sats' },
      idempotent: false,
      payerActorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      postId,
      receipt: {
        amount: { amount: 15, asset: 'sats' },
        paymentEvent: {
          externalRef: 'external.payment.redacted.route_direct_tip_1',
          paymentMode: 'live',
          providerRef: 'provider.mdk_agent_wallet.redacted',
          settlementAuthority: 'recipient_wallet_direct',
          status: 'confirmed',
        },
        tipSettlement: {
          creatorReceivedSpendableValue: true,
          settlementAuthority: 'recipient_wallet_direct',
          state: 'settled',
        },
      },
      recipientActorRef: 'actor.route-test',
      status: 'settled',
    })
    expect(store.challenges).toHaveLength(0)
    expect(store.directTipAttempts).toHaveLength(1)
    expect(store.receipts).toHaveLength(1)
    expect(store.paymentEvents).toHaveLength(1)

    const lookupResponse = await route(
      store,
      `/api/forum/direct-tips/${encodeURIComponent(body.attemptId)}`,
    )
    const lookup = await lookupResponse.json()

    expect(lookupResponse.status).toBe(200)
    expect(lookup).toMatchObject({
      attemptId: body.attemptId,
      receipt: {
        receiptRef: body.receipt.receiptRef,
      },
      status: 'settled',
    })
  })

  test('keeps direct-tip failure explicit without public receipt stats', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    const postId = '66666666-6666-4666-8666-666666666666'
    const response = await route(
      store,
      `/api/forum/posts/${encodeURIComponent(postId)}/direct-tips`,
      {
        body: {
          amount: { amount: 15, asset: 'sats' },
          paymentEvidence: {
            externalRef: 'external.payment.redacted.route_direct_tip_failed',
            paymentMode: 'live',
            providerRef: 'provider.mdk_agent_wallet.redacted',
            redactedEvidenceRef:
              'evidence.payment.redacted.route_direct_tip_failed',
            status: 'failed',
          },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-direct-tip-route-failed',
        },
        method: 'POST',
      },
    )
    const body = await response.json()
    const postDetailResponse = await route(
      store,
      `/api/forum/posts/${encodeURIComponent(postId)}`,
    )
    const postDetail = (await postDetailResponse.json()) as {
      post: {
        tipStats: {
          tipCount: number
          totalPaidSats: number
          totalSettledSats: number
        }
      }
    }

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      receipt: null,
      status: 'failed',
    })
    expect(store.directTipAttempts).toHaveLength(1)
    expect(store.receipts).toHaveLength(0)
    expect(postDetail.post.tipStats).toStrictEqual({
      staleness: {
        composition: 'live_at_read',
        contractVersion: 'projection_staleness.v1',
        maxStalenessSeconds: 0,
        rebuildsOn: [
          'forum_payment_event_confirmed',
          'forum_tip_settlement_claimed',
          'tip_ladder_pay_in_paid',
        ],
      },
      tipCount: 0,
      totalCreditedSats: 0,
      totalPaidSats: 0,
      totalSettledSats: 0,
    })
  })

  test('keeps refunded and reversed direct tips explicit without public settled stats', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    const postId = '66666666-6666-4666-8666-666666666666'

    for (const status of ['refunded', 'reversed'] as const) {
      const response = await route(
        store,
        `/api/forum/posts/${encodeURIComponent(postId)}/direct-tips`,
        {
          body: {
            amount: { amount: 15, asset: 'sats' },
            paymentEvidence: {
              externalRef: `external.payment.redacted.route_direct_tip_${status}`,
              paymentMode: 'live',
              providerRef: 'provider.mdk_agent_wallet.redacted',
              redactedEvidenceRef: `evidence.payment.redacted.route_direct_tip_${status}`,
              status,
            },
          },
          headers: {
            authorization: 'Bearer oa_agent_route_test',
            'idempotency-key': `forum-direct-tip-route-${status}`,
          },
          method: 'POST',
        },
      )
      const body = (await response.json()) as {
        attemptId: string
        paymentEvidence: { status: string }
        receipt: unknown
        status: string
      }
      const lookupResponse = await route(
        store,
        `/api/forum/direct-tips/${encodeURIComponent(body.attemptId)}`,
      )
      const lookup = await lookupResponse.json()

      expect(response.status).toBe(201)
      expect(body).toMatchObject({
        paymentEvidence: { status },
        receipt: null,
        status: 'failed',
      })
      expect(lookupResponse.status).toBe(200)
      expect(lookup).toMatchObject({
        attemptId: body.attemptId,
        paymentEvidence: { status },
        receipt: null,
        status: 'failed',
      })
    }

    const postDetailResponse = await route(
      store,
      `/api/forum/posts/${encodeURIComponent(postId)}`,
    )
    const postDetail = (await postDetailResponse.json()) as {
      post: {
        tipStats: {
          tipCount: number
          totalPaidSats: number
          totalSettledSats: number
        }
      }
    }
    const leaderboardsResponse = await route(
      store,
      '/api/forum/tip-leaderboards',
    )
    const leaderboards = await leaderboardsResponse.json()

    expect(store.directTipAttempts).toHaveLength(2)
    expect(store.receipts).toHaveLength(0)
    expect(store.paymentEvents).toHaveLength(2)
    expect(postDetail.post.tipStats).toStrictEqual({
      staleness: {
        composition: 'live_at_read',
        contractVersion: 'projection_staleness.v1',
        maxStalenessSeconds: 0,
        rebuildsOn: [
          'forum_payment_event_confirmed',
          'forum_tip_settlement_claimed',
          'tip_ladder_pay_in_paid',
        ],
      },
      tipCount: 0,
      totalCreditedSats: 0,
      totalPaidSats: 0,
      totalSettledSats: 0,
    })
    expect(leaderboardsResponse.status).toBe(200)
    expect(leaderboards).toMatchObject({
      creators: [],
      posts: [],
    })
    expect(JSON.stringify(leaderboards)).not.toContain('refunded')
    expect(JSON.stringify(leaderboards)).not.toContain('reversed')
  })

  test('reconciles recovery-pending direct tips from signed MDK webhook events', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    const postId = '66666666-6666-4666-8666-666666666666'
    const pendingResponse = await route(
      store,
      `/api/forum/posts/${encodeURIComponent(postId)}/direct-tips`,
      {
        body: {
          amount: { amount: 15, asset: 'sats' },
          paymentEvidence: {
            externalRef: 'external.payment.redacted.route_direct_tip_observed',
            paymentMode: 'live',
            providerRef: 'provider.mdk_agent_wallet.redacted',
            redactedEvidenceRef:
              'evidence.payment.redacted.route_direct_tip_observed',
            status: 'observed',
          },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-direct-tip-route-observed',
        },
        method: 'POST',
      },
    )
    const pending = (await pendingResponse.json()) as { attemptId: string }
    const webhookBody = JSON.stringify({
      amountSats: 15,
      attemptId: pending.attemptId,
      createdAt: '2026-06-05T20:00:01.000Z',
      id: 'evt_forum_direct_tip_paid_1',
      status: 'paid',
      type: 'payment.succeeded',
    })
    const signature = await signStandardWebhook(
      forumMdkWebhookSecret,
      'evt_forum_direct_tip_paid_1',
      '1780000001',
      webhookBody,
    )
    const webhookResponse = await route(
      store,
      '/api/forum/paid-actions/mdk/webhooks',
      {
        body: JSON.parse(webhookBody),
        headers: {
          'webhook-id': 'evt_forum_direct_tip_paid_1',
          'webhook-signature': signature,
          'webhook-timestamp': '1780000001',
        },
        method: 'POST',
      },
    )
    const replayResponse = await route(
      store,
      '/api/forum/paid-actions/mdk/webhooks',
      {
        body: JSON.parse(webhookBody),
        headers: {
          'webhook-id': 'evt_forum_direct_tip_paid_1',
          'webhook-signature': signature,
          'webhook-timestamp': '1780000001',
        },
        method: 'POST',
      },
    )
    const payerRetryResponse = await route(
      store,
      `/api/forum/posts/${encodeURIComponent(postId)}/direct-tips`,
      {
        body: {
          amount: { amount: 15, asset: 'sats' },
          paymentEvidence: {
            externalRef: 'external.payment.redacted.route_direct_tip_observed',
            paymentMode: 'live',
            providerRef: 'provider.mdk_agent_wallet.redacted',
            redactedEvidenceRef:
              'evidence.payment.redacted.route_direct_tip_observed',
            status: 'observed',
          },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-direct-tip-route-observed',
        },
        method: 'POST',
      },
    )
    const webhook = await webhookResponse.json()
    const replay = await replayResponse.json()
    const payerRetry = await payerRetryResponse.json()
    const postDetailResponse = await route(
      store,
      `/api/forum/posts/${encodeURIComponent(postId)}`,
    )
    const postDetail = (await postDetailResponse.json()) as {
      post: {
        tipStats: {
          tipCount: number
          totalPaidSats: number
          totalSettledSats: number
        }
      }
    }

    expect(pendingResponse.status).toBe(201)
    expect(webhookResponse.status).toBe(201)
    expect(replayResponse.status).toBe(200)
    expect(payerRetryResponse.status).toBe(200)
    expect(webhook).toMatchObject({
      attemptId: pending.attemptId,
      idempotent: false,
      receipt: {
        amount: { amount: 15, asset: 'sats' },
        paymentEvent: {
          providerRef: 'provider.mdk_webhook.dashboard_standard_webhooks',
          settlementAuthority: 'recipient_wallet_direct',
          status: 'confirmed',
        },
      },
      status: 'settled',
    })
    expect(replay).toMatchObject({
      attemptId: pending.attemptId,
      idempotent: true,
      status: 'settled',
    })
    expect(payerRetry).toMatchObject({
      attemptId: pending.attemptId,
      idempotent: true,
      receipt: {
        paymentEvent: {
          providerRef: 'provider.mdk_webhook.dashboard_standard_webhooks',
          status: 'confirmed',
        },
      },
      status: 'settled',
    })
    expect(store.directTipWebhookEvents[0]?.delivery_count).toBe(2)
    expect(store.receipts).toHaveLength(1)
    expect(postDetail.post.tipStats).toStrictEqual({
      staleness: {
        composition: 'live_at_read',
        contractVersion: 'projection_staleness.v1',
        maxStalenessSeconds: 0,
        rebuildsOn: [
          'forum_payment_event_confirmed',
          'forum_tip_settlement_claimed',
          'tip_ladder_pay_in_paid',
        ],
      },
      tipCount: 1,
      totalCreditedSats: 0,
      totalPaidSats: 15,
      totalSettledSats: 15,
    })
  })

  test('rejects direct-tip MDK webhooks with an invalid signature', async () => {
    const store = new ForumRouteStore()
    const response = await route(store, '/api/forum/paid-actions/mdk/webhooks', {
      body: {
        amountSats: 15,
        attemptId: 'aaaaaaaa-1111-4111-8111-000000000001',
        id: 'evt_forum_direct_tip_bad_sig',
        status: 'paid',
      },
      headers: {
        'webhook-id': 'evt_forum_direct_tip_bad_sig',
        'webhook-signature': 'bad-signature',
        'webhook-timestamp': '1780000001',
      },
      method: 'POST',
    })

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toMatchObject({
      error: 'mdk_webhook_invalid_signature',
    })
    expect(store.directTipWebhookEvents).toHaveLength(0)
  })

  test('rejects direct-tip MDK webhooks whose amount does not match the attempt', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    const postId = '66666666-6666-4666-8666-666666666666'
    const pendingResponse = await route(
      store,
      `/api/forum/posts/${encodeURIComponent(postId)}/direct-tips`,
      {
        body: {
          amount: { amount: 15, asset: 'sats' },
          paymentEvidence: {
            externalRef:
              'external.payment.redacted.route_direct_tip_wrong_amount',
            paymentMode: 'live',
            providerRef: 'provider.mdk_agent_wallet.redacted',
            redactedEvidenceRef:
              'evidence.payment.redacted.route_direct_tip_wrong_amount',
            status: 'observed',
          },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-direct-tip-route-wrong-amount',
        },
        method: 'POST',
      },
    )
    const pending = (await pendingResponse.json()) as { attemptId: string }
    const webhookBody = JSON.stringify({
      amountSats: 16,
      attemptId: pending.attemptId,
      id: 'evt_forum_direct_tip_wrong_amount',
      status: 'paid',
    })
    const signature = await signStandardWebhook(
      forumMdkWebhookSecret,
      'evt_forum_direct_tip_wrong_amount',
      '1780000001',
      webhookBody,
    )
    const response = await route(store, '/api/forum/paid-actions/mdk/webhooks', {
      body: JSON.parse(webhookBody),
      headers: {
        'webhook-id': 'evt_forum_direct_tip_wrong_amount',
        'webhook-signature': signature,
        'webhook-timestamp': '1780000001',
      },
      method: 'POST',
    })

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toMatchObject({
      error: 'binding_mismatch',
    })
    expect(store.receipts).toHaveLength(0)
  })

  test('rejects direct Forum tips when the target author has no BOLT 12 offer', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(
      readyTipRecipientWalletRow({ bolt12_offer: null }),
    )
    const response = await route(
      store,
      '/api/forum/posts/66666666-6666-4666-8666-666666666666/direct-tips',
      {
        body: {
          amount: { amount: 15, asset: 'sats' },
          paymentEvidence: {
            externalRef: 'external.payment.redacted.route_direct_tip_no_offer',
            paymentMode: 'live',
            providerRef: 'provider.mdk_agent_wallet.redacted',
            redactedEvidenceRef:
              'evidence.payment.redacted.route_direct_tip_no_offer',
            status: 'confirmed',
          },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-direct-tip-route-no-offer',
        },
        method: 'POST',
      },
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toMatchObject({
      error: 'recipient_not_ready',
    })
    expect(store.receipts).toHaveLength(0)
    expect(store.paymentEvents).toHaveLength(0)
  })

  test('blocks custom-amount rewards from the old hosted L402 path', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    const postId = '66666666-6666-4666-8666-666666666666'
    const path = `/api/forum/posts/${postId}/rewards`
    const customAmountPreviewResponse = await route(store, path, {
      body: {
        amount: { amount: 15, asset: 'sats' },
        requestBodyDigest: 'sha256:forum-reward-body-custom-amount',
        spendCap: { amount: 15, asset: 'sats' },
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'forum-paid-reward-preview-custom-amount',
      },
      method: 'POST',
    })

    expect(customAmountPreviewResponse.status).toBe(200)
    await expect(customAmountPreviewResponse.json()).resolves.toMatchObject({
      challenge: null,
      paymentRequired: false,
      writeDenial: {
        denialKind: 'payment_required',
        denialRef: 'blocker.public.forum_tip.bolt12_direct_required',
        payable: false,
      },
    })
    expect(store.challenges).toHaveLength(0)
  })

  test('lets the receipt recipient claim spendable settlement evidence', async () => {
    const store = new ForumRouteStore()
    const postId = '66666666-6666-4666-8666-666666666666'
    const topicId = '55555555-5555-4555-8555-555555555555'
    const receiptRef = 'receipt.forum.route_settled'
    const recipientActorRef = 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'

    store.receipts.push({
      action_kind: 'post_reward',
      amount_asset: 'sats',
      amount_value: 100,
      archived_at: null,
      created_at: '2026-06-05T20:00:00.000Z',
      id: 'receipt-route-settled',
      public_projection_json: projectionJson,
      receipt_ref: receiptRef,
      recipient_actor_ref: recipientActorRef,
      redacted_payment_ref: 'payment_proof.public.route_settled',
      target_forum_id: '33333333-3333-4333-8333-333333333333',
      target_post_id: postId,
      target_topic_id: topicId,
    })
    store.moneyActions.push({
      action_kind: 'post_reward',
      amount_asset: 'sats',
      amount_value: 100,
      earning_actor_ref: recipientActorRef,
      id: 'money-action-route-settled',
      payment_event_id: 'payment-event-route-settled',
      public_projection_json: projectionJson,
      receipt_id: 'receipt-route-settled',
    })
    store.paymentEvents.push({
      amount_asset: 'sats',
      amount_value: 100,
      archived_at: null,
      created_at: '2026-06-05T20:00:00.000Z',
      external_ref: 'external.forum_l402.route_settled',
      id: 'payment-event-route-settled',
      money_action_id: 'money-action-route-settled',
      provider_ref: 'provider.forum.route.mdk.sandbox',
      public_projection_json: JSON.stringify({
        actionKind: 'post_reward',
        amount: { amount: 100, asset: 'sats' },
        challengeId: '77777777-7777-4777-8777-777777777777',
        createdAt: '2026-06-05T20:00:00.000Z',
        externalRef: 'external.forum_l402.route_settled',
        payerActorRef: 'agent:payer_route_test',
        paymentEventRef: 'payment_event.forum.route_settled',
        paymentMode: 'sandbox',
        providerRef: 'provider.forum.route.mdk.sandbox',
        receiptRef,
        recipientActorRef,
        redactedEvidenceRef: 'evidence.forum_l402.route_settled',
        settlementAuthority: 'recipient_wallet_direct',
        status: 'confirmed',
      }),
      redacted_evidence_ref: 'evidence.forum_l402.route_settled',
    })

    const claimResponse = await route(
      store,
      `/api/forum/receipts/${encodeURIComponent(receiptRef)}/settlement-claims`,
      {
        body: {
          settlementEvidenceRefs: [
            'settlement_evidence.public.mdk_agent_wallet.receive_confirmed',
          ],
          settlementRef: 'settlement.public.route_test.creator_wallet.receipt',
          sourceRef: 'source.public.route_test.agent_wallet',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-settlement-claim-1',
        },
        method: 'POST',
      },
    )
    const claim = await claimResponse.json()
    const replayResponse = await route(
      store,
      `/api/forum/receipts/${encodeURIComponent(receiptRef)}/settlement-claims`,
      {
        body: {
          settlementEvidenceRefs: [
            'settlement_evidence.public.mdk_agent_wallet.receive_confirmed',
          ],
          settlementRef: 'settlement.public.route_test.creator_wallet.receipt',
          sourceRef: 'source.public.route_test.agent_wallet',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-settlement-claim-1',
        },
        method: 'POST',
      },
    )
    const receiptResponse = await route(
      store,
      `/api/forum/receipts/${encodeURIComponent(receiptRef)}`,
    )
    const receipt = await receiptResponse.json()
    const earningsResponse = await route(
      store,
      `/api/forum/actors/${encodeURIComponent(recipientActorRef)}/tip-earnings`,
    )
    const earnings = await earningsResponse.json()
    const postDetailResponse = await route(
      store,
      `/api/forum/posts/${encodeURIComponent(postId)}`,
    )
    const postDetail = await postDetailResponse.json()

    expect(claimResponse.status).toBe(201)
    expect(claim).toMatchObject({
      idempotent: false,
      receipt: {
        receiptRef,
        settlementClaim: {
          receiptRef,
          recipientActorRef,
          settlementEvidenceRefs: [
            'settlement_evidence.public.mdk_agent_wallet.receive_confirmed',
          ],
          settlementRef: 'settlement.public.route_test.creator_wallet.receipt',
          sourceRef: 'source.public.route_test.agent_wallet',
        },
        tipSettlement: {
          creatorReceivedSpendableValue: true,
          recipientSettlementEvidence: true,
          settlementAuthority: 'recipient_wallet_direct',
          state: 'settled',
        },
      },
    })
    expect(store.tipSettlementClaims).toHaveLength(1)
    expect(replayResponse.status).toBe(200)
    expect(await replayResponse.json()).toMatchObject({ idempotent: true })
    expect(receipt).toMatchObject({
      settlementClaim: {
        receiptRef,
        recipientActorRef,
      },
      tipSettlement: {
        creatorReceivedSpendableValue: true,
        state: 'settled',
      },
    })
    expect(earnings).toMatchObject({
      summary: {
        paidCount: 0,
        settledCount: 1,
        totalPaidSats: 100,
        totalSettledSats: 100,
      },
    })
    expect(postDetail).toMatchObject({
      post: {
        tipStats: {
          tipCount: 1,
          totalPaidSats: 100,
          totalSettledSats: 100,
        },
      },
    })
    expect(JSON.stringify(claim)).not.toContain('lnbc')
    expect(JSON.stringify(claim)).not.toContain('preimage')
  })

  test('rejects settlement claims from a non-recipient agent', async () => {
    const store = new ForumRouteStore()
    store.receipts.push({
      action_kind: 'post_reward',
      amount_asset: 'sats',
      amount_value: 100,
      archived_at: null,
      created_at: '2026-06-05T20:00:00.000Z',
      id: 'receipt-wrong-recipient',
      public_projection_json: projectionJson,
      receipt_ref: 'receipt.forum.wrong_recipient',
      recipient_actor_ref: 'agent:bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
      redacted_payment_ref: 'payment_proof.public.wrong_recipient',
      target_forum_id: '33333333-3333-4333-8333-333333333333',
      target_post_id: '66666666-6666-4666-8666-666666666666',
      target_topic_id: '55555555-5555-4555-8555-555555555555',
    })

    const response = await route(
      store,
      '/api/forum/receipts/receipt.forum.wrong_recipient/settlement-claims',
      {
        body: {
          settlementEvidenceRefs: [
            'settlement_evidence.public.mdk_agent_wallet.receive_confirmed',
          ],
          settlementRef: 'settlement.public.route_test.creator_wallet.receipt',
          sourceRef: 'source.public.route_test.agent_wallet',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-settlement-claim-wrong-recipient',
        },
        method: 'POST',
      },
    )

    expect(response.status).toBe(403)
    expect(await response.json()).toMatchObject({ error: 'forbidden' })
    expect(store.tipSettlementClaims).toHaveLength(0)
  })

  test('requires confirmed payer payment evidence before settlement claim', async () => {
    const store = new ForumRouteStore()
    store.receipts.push({
      action_kind: 'post_reward',
      amount_asset: 'sats',
      amount_value: 100,
      archived_at: null,
      created_at: '2026-06-05T20:00:00.000Z',
      id: 'receipt-unconfirmed',
      public_projection_json: projectionJson,
      receipt_ref: 'receipt.forum.unconfirmed',
      recipient_actor_ref: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      redacted_payment_ref: 'payment_proof.public.unconfirmed',
      target_forum_id: '33333333-3333-4333-8333-333333333333',
      target_post_id: '66666666-6666-4666-8666-666666666666',
      target_topic_id: '55555555-5555-4555-8555-555555555555',
    })

    const response = await route(
      store,
      '/api/forum/receipts/receipt.forum.unconfirmed/settlement-claims',
      {
        body: {
          settlementEvidenceRefs: [
            'settlement_evidence.public.mdk_agent_wallet.receive_confirmed',
          ],
          settlementRef: 'settlement.public.route_test.creator_wallet.receipt',
          sourceRef: 'source.public.route_test.agent_wallet',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-settlement-claim-unconfirmed',
        },
        method: 'POST',
      },
    )

    expect(response.status).toBe(402)
    expect(await response.json()).toMatchObject({
      error: 'payment_verification_failed',
    })
    expect(store.tipSettlementClaims).toHaveLength(0)
  })

  test('projects operator tip reconciliation for refunded and reversed rewards', async () => {
    const store = new ForumRouteStore()
    const postId = '66666666-6666-4666-8666-666666666666'
    const topicId = '55555555-5555-4555-8555-555555555555'

    store.receipts.push(
      {
        action_kind: 'post_reward',
        amount_asset: 'sats',
        amount_value: 100,
        archived_at: null,
        created_at: '2026-06-05T20:00:00.000Z',
        id: 'receipt-refunded',
        public_projection_json: projectionJson,
        receipt_ref: 'receipt.forum.refunded',
        recipient_actor_ref: 'actor.route-test',
        redacted_payment_ref: 'payment_proof.public.refunded',
        target_forum_id: '33333333-3333-4333-8333-333333333333',
        target_post_id: postId,
        target_topic_id: topicId,
      },
      {
        action_kind: 'post_reward',
        amount_asset: 'sats',
        amount_value: 100,
        archived_at: null,
        created_at: '2026-06-05T20:01:00.000Z',
        id: 'receipt-reversed',
        public_projection_json: projectionJson,
        receipt_ref: 'receipt.forum.reversed',
        recipient_actor_ref: 'actor.route-test',
        redacted_payment_ref: 'payment_proof.public.reversed',
        target_forum_id: '33333333-3333-4333-8333-333333333333',
        target_post_id: postId,
        target_topic_id: topicId,
      },
    )
    store.moneyActions.push(
      {
        action_kind: 'post_reward',
        amount_asset: 'sats',
        amount_value: 100,
        earning_actor_ref: 'actor.route-test',
        id: 'money-action-refunded',
        payment_event_id: 'payment-event-refunded',
        public_projection_json: projectionJson,
        receipt_id: 'receipt-refunded',
      },
      {
        action_kind: 'post_reward',
        amount_asset: 'sats',
        amount_value: 100,
        earning_actor_ref: 'actor.route-test',
        id: 'money-action-reversed',
        payment_event_id: 'payment-event-reversed',
        public_projection_json: projectionJson,
        receipt_id: 'receipt-reversed',
      },
    )
    store.paymentEvents.push(
      {
        amount_asset: 'sats',
        amount_value: 100,
        archived_at: null,
        created_at: '2026-06-05T20:00:00.000Z',
        external_ref: 'external.payment.redacted.refunded',
        id: 'payment-event-refunded',
        money_action_id: 'money-action-refunded',
        provider_ref: 'provider.mdk_l402.redacted',
        public_projection_json: JSON.stringify({
          actionKind: 'post_reward',
          amount: { amount: 100, asset: 'sats' },
          challengeId: '77777777-7777-4777-8777-777777777777',
          createdAt: '2026-06-05T20:00:00.000Z',
          externalRef: 'external.payment.redacted.refunded',
          payerActorRef: 'actor.alice',
          paymentEventRef: 'payment-event-refunded',
          paymentMode: 'signet',
          providerRef: 'provider.mdk_l402.redacted',
          receiptRef: 'receipt.forum.refunded',
          recipientActorRef: 'actor.route-test',
          redactedEvidenceRef: 'evidence.payment.redacted.refunded',
          status: 'refunded',
        }),
        redacted_evidence_ref: 'evidence.payment.redacted.refunded',
      },
      {
        amount_asset: 'sats',
        amount_value: 100,
        archived_at: null,
        created_at: '2026-06-05T20:01:00.000Z',
        external_ref: 'external.payment.redacted.reversed',
        id: 'payment-event-reversed',
        money_action_id: 'money-action-reversed',
        provider_ref: 'provider.mdk_l402.redacted',
        public_projection_json: JSON.stringify({
          actionKind: 'post_reward',
          amount: { amount: 100, asset: 'sats' },
          challengeId: '88888888-8888-4888-8888-888888888888',
          createdAt: '2026-06-05T20:01:00.000Z',
          externalRef: 'external.payment.redacted.reversed',
          payerActorRef: 'actor.alice',
          paymentEventRef: 'payment-event-reversed',
          paymentMode: 'signet',
          providerRef: 'provider.mdk_l402.redacted',
          receiptRef: 'receipt.forum.reversed',
          recipientActorRef: 'actor.route-test',
          redactedEvidenceRef: 'evidence.payment.redacted.reversed',
          status: 'reversed',
        }),
        redacted_evidence_ref: 'evidence.payment.redacted.reversed',
      },
    )

    const unauthorized = await route(
      store,
      '/api/forum/moderation/tip-earnings?actorRef=actor.route-test',
    )
    const response = await route(
      store,
      '/api/forum/moderation/tip-earnings?actorRef=actor.route-test',
      {
        moderator: 'admin',
      },
    )
    const body = await response.json()

    expect(unauthorized.status).toBe(401)
    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      acceptedWorkPayoutBoundary: 'ordinary_forum_tips_are_not_accepted_work',
      actorRef: 'actor.route-test',
      earnings: expect.arrayContaining([
        expect.objectContaining({
          acceptedWorkPayoutEvidence: false,
          paymentState: 'refunded',
          settlementState: 'refunded',
          targetPostPermalink:
            'https://openagents.com/forum/t/55555555-5555-4555-8555-555555555555#post-66666666-6666-4666-8666-666666666666',
        }),
        expect.objectContaining({
          acceptedWorkPayoutEvidence: false,
          paymentState: 'reversed',
          settlementState: 'reversed',
        }),
      ]),
      summary: {
        refundedCount: 1,
        reversedCount: 1,
        totalCount: 2,
        totalPaidSats: 0,
        totalSettledSats: 0,
      },
    })
    expect(JSON.stringify(body)).not.toContain('lnbc')
    expect(JSON.stringify(body)).not.toContain('preimage')
    expect(JSON.stringify(body)).not.toContain('payout_target.raw')
  })

  test('returns private non-tip Forum payment payload only to the challenge actor', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    const postId = '66666666-6666-4666-8666-666666666666'
    const path = `/api/forum/posts/${postId}/boosts`
    const previewResponse = await route(store, path, {
      body: {
        requestBodyDigest: 'sha256:forum-boost-private-payment-body',
        spendCap: { amount: 100, asset: 'sats' },
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'forum-paid-reward-private-payment-preview',
      },
      method: 'POST',
    })
    const preview = (await previewResponse.json()) as Readonly<{
      challenge: Readonly<{ challengeId: string }>
    }>
    const privatePaymentResponse = await route(
      store,
      '/api/forum/paid-actions/private-payment',
      {
        body: {
          challengeId: preview.challenge.challengeId,
          method: 'POST',
          path,
          requestBodyDigest: 'sha256:forum-boost-private-payment-body',
          routeParams: { postId },
          spendCap: { amount: 100, asset: 'sats' },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
        },
        method: 'POST',
      },
    )
    const privatePayment = await privatePaymentResponse.json()

    expect(previewResponse.status).toBe(200)
    expect(JSON.stringify(preview)).not.toContain('lntbs')
    expect(JSON.stringify(preview)).not.toContain('oa-l402-v1.')
    expect(privatePaymentResponse.status).toBe(200)
    expect(privatePayment).toMatchObject({
      challenge: { challengeId: preview.challenge.challengeId },
      privatePayment: {
        bolt11: expect.stringMatching(/^lntbs/i),
        credential: expect.stringMatching(/^oa-l402-v1\./),
        l402ProofRef: expect.stringContaining('payment_proof.public'),
        provider: 'mdk_hosted',
        sandbox: true,
      },
    })

    const storedChallenge = store.challenges[0]
    expect(storedChallenge).toBeDefined()
    store.challenges[0] = {
      ...storedChallenge!,
      actor_ref: 'agent:someone_else',
    }
    const unauthorized = await route(
      store,
      '/api/forum/paid-actions/private-payment',
      {
        body: {
          challengeId: preview.challenge.challengeId,
          method: 'POST',
          path,
          requestBodyDigest: 'sha256:forum-boost-private-payment-body',
          routeParams: { postId },
          spendCap: { amount: 100, asset: 'sats' },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
        },
        method: 'POST',
      },
    )

    expect(unauthorized.status).toBe(409)
    await expect(unauthorized.json()).resolves.toMatchObject({
      error: 'actor_mismatch',
    })
  })

  test('rejects malformed non-tip Forum L402 credentials before receipt creation', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    const postId = '66666666-6666-4666-8666-666666666666'
    const path = `/api/forum/posts/${postId}/boosts`
    const previewResponse = await route(store, path, {
      body: {
        requestBodyDigest: 'sha256:forum-boost-body-invalid-l402',
        spendCap: { amount: 100, asset: 'sats' },
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'forum-paid-reward-preview-invalid-l402',
      },
      method: 'POST',
    })
    const preview = (await previewResponse.json()) as Readonly<{
      challenge: Readonly<{ challengeId: string }>
    }>
    const paymentProofRef = 'payment_proof.public.forum_reward.invalid_l402'
    const redeemResponse = await route(
      store,
      '/api/forum/paid-actions/redeem',
      {
        body: {
          challengeId: preview.challenge.challengeId,
          l402ProofRef: paymentProofRef,
          method: 'POST',
          path,
          requestBodyDigest: 'sha256:forum-boost-body-invalid-l402',
          routeParams: { postId },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-paid-reward-redeem-invalid-l402',
          'x-openagents-l402': `oa-l402-v1.invalid.signature:${paymentProofRef}`,
        },
        method: 'POST',
      },
    )

    expect(previewResponse.status).toBe(200)
    expect(redeemResponse.status).toBe(402)
    await expect(redeemResponse.json()).resolves.toMatchObject({
      error: 'payment_verification_failed',
      reason: 'reason.l402_credential.malformed',
    })
    expect(store.receipts).toHaveLength(0)
    expect(store.moneyActions).toHaveLength(0)
    expect(store.paymentEvents).toHaveLength(0)
  })

  test('blocks Forum reward preview when the post author is not recipient-ready', async () => {
    const store = new ForumRouteStore()
    const postId = '66666666-6666-4666-8666-666666666666'
    const response = await route(store, `/api/forum/posts/${postId}/rewards`, {
      body: {
        requestBodyDigest: 'sha256:forum-reward-body',
        spendCap: { amount: 100, asset: 'sats' },
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'forum-paid-reward-preview-recipient-missing',
      },
      method: 'POST',
    })
    const body = await response.json()

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      challenge: null,
      paymentRequired: false,
      writeDenial: {
        denialKind: 'recipient_not_ready',
        denialRef: 'blocker.public.forum_tip_recipient.wallet_missing',
        payable: false,
      },
    })
    expect(store.challenges).toHaveLength(0)
  })

  test('blocks self-tipping before issuing a Forum reward challenge', async () => {
    const store = new ForumRouteStore()
    store.posts[0] = {
      ...store.posts[0]!,
      actor_json: authenticatedAgentActorJson,
    }
    store.tipRecipientWallets.push(
      readyTipRecipientWalletRow({
        actor_ref: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
        readiness_refs_json: JSON.stringify([
          'readiness.public.forum_tip_recipient.authenticated_agent',
        ]),
        wallet_ref: 'wallet.public.forum_tip_recipient.authenticated_agent',
      }),
    )
    const postId = '66666666-6666-4666-8666-666666666666'
    const response = await route(store, `/api/forum/posts/${postId}/rewards`, {
      body: {
        requestBodyDigest: 'sha256:forum-reward-body',
        spendCap: { amount: 100, asset: 'sats' },
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'forum-paid-reward-preview-self-tip',
      },
      method: 'POST',
    })
    const body = await response.json()

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      challenge: null,
      paymentRequired: false,
      writeDenial: {
        denialKind: 'safety_denied',
        denialRef: 'policy.public.forum_tip.self_tipping_blocked',
        payable: false,
      },
    })
    expect(store.challenges).toHaveLength(0)
  })

  test('keeps repeated Forum reward previews blocked before L402 challenge creation', async () => {
    const store = new ForumRouteStore()
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    const postId = '66666666-6666-4666-8666-666666666666'

    for (let index = 0; index < 6; index += 1) {
      const response = await route(
        store,
        `/api/forum/posts/${postId}/rewards`,
        {
          body: {
            requestBodyDigest: `sha256:forum-reward-body-${index}`,
            spendCap: { amount: 100, asset: 'sats' },
          },
          headers: {
            authorization: 'Bearer oa_agent_route_test',
            'idempotency-key': `forum-paid-reward-preview-rate-${index}`,
          },
          method: 'POST',
        },
      )
      await expect(response.json()).resolves.toMatchObject({
        challenge: null,
        paymentRequired: false,
        writeDenial: {
          denialKind: 'payment_required',
          denialRef: 'blocker.public.forum_tip.bolt12_direct_required',
          payable: false,
        },
      })
    }

    const overLimit = await route(store, `/api/forum/posts/${postId}/rewards`, {
      body: {
        requestBodyDigest: 'sha256:forum-reward-body-over-limit',
        spendCap: { amount: 100, asset: 'sats' },
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'forum-paid-reward-preview-rate-over-limit',
      },
      method: 'POST',
    })
    const overLimitBody = await overLimit.json()
    const overCap = await route(store, `/api/forum/posts/${postId}/rewards`, {
      body: {
        requestBodyDigest: 'sha256:forum-reward-body-over-cap',
        spendCap: { amount: 9, asset: 'sats' },
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'forum-paid-reward-preview-over-cap',
      },
      method: 'POST',
    })

    expect(overLimit.status).toBe(200)
    expect(overLimitBody).toMatchObject({
      challenge: null,
      paymentRequired: false,
      writeDenial: {
        denialKind: 'payment_required',
        denialRef: 'blocker.public.forum_tip.bolt12_direct_required',
        payable: false,
      },
    })
    expect(overCap.status).toBe(400)
    expect(store.challenges).toHaveLength(0)
  })

  test('previews down-signals without assigning author earnings', async () => {
    const store = new ForumRouteStore()
    const postId = '66666666-6666-4666-8666-666666666666'
    const response = await route(
      store,
      `/api/forum/posts/${postId}/down-signals`,
      {
        body: {
          requestBodyDigest: 'sha256:forum-down-signal-body',
          spendCap: { amount: 100, asset: 'sats' },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-paid-down-signal-preview-1',
        },
        method: 'POST',
      },
    )

    await expect(response.json()).resolves.toMatchObject({
      challenge: {
        actionKind: 'post_down_signal',
        price: { amount: 100, asset: 'sats' },
        target: { postId },
      },
      paymentRequired: true,
    })
  })

  test('does not let payment buy missing auth or hidden/tombstoned content access', async () => {
    const store = new ForumRouteStore()
    const postId = '66666666-6666-4666-8666-666666666666'
    const noAuth = await route(store, `/api/forum/posts/${postId}/rewards`, {
      body: {
        requestBodyDigest: 'sha256:forum-reward-body',
        spendCap: { amount: 100, asset: 'sats' },
      },
      headers: {
        'idempotency-key': 'forum-paid-reward-preview-no-auth',
      },
      method: 'POST',
    })

    store.posts[0] = { ...store.posts[0]!, state: 'hidden' }
    const hidden = await route(store, `/api/forum/posts/${postId}/rewards`, {
      body: {
        requestBodyDigest: 'sha256:forum-reward-body',
        spendCap: { amount: 100, asset: 'sats' },
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'forum-paid-reward-preview-hidden',
      },
      method: 'POST',
    })

    store.posts[0] = { ...store.posts[0]!, state: 'held_for_review' }
    const held = await route(store, `/api/forum/posts/${postId}/rewards`, {
      body: {
        requestBodyDigest: 'sha256:forum-reward-body',
        spendCap: { amount: 100, asset: 'sats' },
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'forum-paid-reward-preview-held',
      },
      method: 'POST',
    })

    store.posts[0] = { ...store.posts[0]!, state: 'tombstoned' }
    store.tipRecipientWallets.push(readyTipRecipientWalletRow())
    const tombstoned = await route(
      store,
      `/api/forum/posts/${postId}/rewards`,
      {
        body: {
          requestBodyDigest: 'sha256:forum-reward-body',
          spendCap: { amount: 100, asset: 'sats' },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'forum-paid-reward-preview-tombstoned',
        },
        method: 'POST',
      },
    )

    expect(noAuth.status).toBe(401)
    expect(hidden.status).toBe(404)
    expect(held.status).toBe(404)
    expect(tombstoned.status).toBe(404)
    expect(store.challenges).toHaveLength(0)
  })

  test('search excludes void by default and requires auth for unlisted discovery', async () => {
    const store = new ForumRouteStore()
    const topicResponse = await route(store, '/api/forum/forums/void/topics', {
      body: {
        bodyText: 'Hello world from the void test lane.',
        title: 'Hello world',
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'void-search-topic',
      },
      method: 'POST',
    })
    const topicBody = (await topicResponse.json()) as Readonly<{
      topic: Readonly<{ topicId: string }>
    }>
    const defaultSearch = await route(store, '/api/forum/search?q=Hello')
    const unauthorizedSearch = await route(
      store,
      '/api/forum/search?q=Hello&include=unlisted',
    )
    const authedSearch = await route(
      store,
      '/api/forum/search?q=Hello&include=unlisted',
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
        },
      },
    )
    const exactTopic = await route(
      store,
      `/api/forum/topics/${topicBody.topic.topicId}`,
    )

    await expect(defaultSearch.json()).resolves.toMatchObject({
      forums: [],
      posts: [],
      topics: [],
    })
    expect(unauthorizedSearch.status).toBe(401)
    await expect(authedSearch.json()).resolves.toMatchObject({
      includeUnlisted: true,
      posts: [{ bodyText: 'Hello world from the void test lane.' }],
      query: 'Hello',
      topics: [{ title: 'Hello world' }],
    })
    await expect(exactTopic.json()).resolves.toMatchObject({
      posts: [{ bodyText: 'Hello world from the void test lane.' }],
      topic: { title: 'Hello world' },
    })
  })

  test('topic detail resolves by slug as well as topicId', async () => {
    const store = new ForumRouteStore()
    const createResponse = await route(
      store,
      '/api/forum/forums/void/topics',
      {
        body: {
          bodyText: 'Pretty slug URLs should resolve like topicId URLs.',
          title: 'Slug resolution works',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'slug-resolution-topic',
        },
        method: 'POST',
      },
    )
    const createBody = (await createResponse.json()) as Readonly<{
      topic: Readonly<{ slug: string; topicId: string }>
    }>
    const topicId = createBody.topic.topicId
    const slug = createBody.topic.slug

    expect(createResponse.status).toBe(201)
    expect(typeof slug).toBe('string')
    expect(slug.length).toBeGreaterThan(0)
    expect(createBody).toMatchObject({
      topicHref: `/forum/t/${topicId}`,
      webUrl: `/forum/t/${topicId}`,
    })
    // The created slug must differ from the topicId so the two lookup forms are
    // genuinely distinct (otherwise the test would not exercise slug
    // resolution).
    expect(slug).not.toBe(topicId)

    const byId = await route(store, `/api/forum/topics/${topicId}`)
    const bySlug = await route(
      store,
      `/api/forum/topics/${encodeURIComponent(slug)}`,
    )
    const unknown = await route(
      store,
      '/api/forum/topics/this-ref-does-not-exist',
    )

    expect(byId.status).toBe(200)
    expect(bySlug.status).toBe(200)
    // Unknown ref (neither topicId nor slug) must be a clean not-found, never a
    // 500.
    expect(unknown.status).toBe(404)

    const byIdBody = (await byId.json()) as Readonly<{
      topic: Readonly<{ slug: string; title: string; topicId: string }>
    }>
    const bySlugBody = (await bySlug.json()) as Readonly<{
      topic: Readonly<{ slug: string; title: string; topicId: string }>
    }>

    // Both URL forms must resolve to the very same topic.
    expect(bySlugBody.topic.topicId).toBe(topicId)
    expect(bySlugBody.topic.topicId).toBe(byIdBody.topic.topicId)
    expect(bySlugBody.topic.slug).toBe(slug)
    expect(bySlugBody.topic.title).toBe('Slug resolution works')
    expect(byIdBody).toMatchObject({
      topicHref: `/forum/t/${topicId}`,
      webUrl: `/forum/t/${topicId}`,
    })
    expect(bySlugBody).toMatchObject({
      topicHref: `/forum/t/${topicId}`,
      webUrl: `/forum/t/${topicId}`,
    })
  })

  test('search only returns listed public visible content', async () => {
    const store = new ForumRouteStore()
    const listed = await route(store, '/api/forum/search?q=First')

    store.forums[0] = { ...store.forums[0]!, discoverability: 'hidden' }
    const hiddenForum = await route(store, '/api/forum/search?q=First')

    store.forums[0] = {
      ...store.forums[0]!,
      discoverability: 'listed',
      visibility: 'team',
    }
    const privateForum = await route(store, '/api/forum/search?q=First')

    store.forums[0] = {
      ...store.forums[0]!,
      discoverability: 'listed',
      visibility: 'public',
    }
    store.topics[0] = { ...store.topics[0]!, state: 'hidden' }
    const hiddenTopic = await route(store, '/api/forum/search?q=First')

    store.topics[0] = { ...store.topics[0]!, state: 'open' }
    store.posts[0] = { ...store.posts[0]!, state: 'hidden' }
    const hiddenPost = await route(store, '/api/forum/search?q=Seed')

    await expect(listed.json()).resolves.toMatchObject({
      topics: [{ title: 'First Topic' }],
    })
    await expect(hiddenForum.json()).resolves.toMatchObject({
      forums: [],
      posts: [],
      topics: [],
    })
    await expect(privateForum.json()).resolves.toMatchObject({
      forums: [],
      posts: [],
      topics: [],
    })
    await expect(hiddenTopic.json()).resolves.toMatchObject({
      topics: [],
    })
    await expect(hiddenPost.json()).resolves.toMatchObject({
      posts: [],
    })
  })

  test('returns public Forum launch gate status', async () => {
    const store = new ForumRouteStore()
    const response = await route(store, '/api/forum/launch-status')
    const body = (await response.json()) as Readonly<{
      gates: ReadonlyArray<
        Readonly<{
          id: string
          severity: string
          state: string
        }>
      >
      publicPosting: Readonly<{ listedForums: string; voidLane: string }>
      publicTipping: Readonly<{
        onboarding: Readonly<{
          payerReadiness: Readonly<{
            blockerRefs: ReadonlyArray<string>
            state: string
            tippingSpendAllowed: boolean
          }>
          recipientStateRefs: ReadonlyArray<string>
          settlementStateRefs: ReadonlyArray<string>
        }>
        postTips: string
        remainingBeforeLiveTips: ReadonlyArray<string>
      }>
      status: string
    }>

    expect(response.status).toBe(200)
    expect(body).toMatchObject({
      publicPosting: {
        listedForums: 'ready',
        voidLane: 'degraded',
      },
      publicTipping: {
        onboarding: {
          payerReadiness: {
            blockerRefs: [],
            state: 'send_ready',
            tippingSpendAllowed: true,
          },
          recipientStateRefs: [
            'state.public.forum_post_tip.recipient_missing',
            'state.public.forum_post_tip.recipient_receive_ready',
          ],
          settlementStateRefs: [
            'state.public.forum_post_tip.paid_pending_settlement',
            'state.public.forum_post_tip.settled',
          ],
        },
        postTips: 'ready',
        remainingBeforeLiveTips: [],
      },
      status: 'ready',
    })
    expect(body.gates).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'payment_redaction',
          severity: 'required',
          state: 'ready',
        }),
        expect.objectContaining({
          id: 'default_rate_limit_policy',
          severity: 'recommended',
          state: 'ready',
        }),
        expect.objectContaining({
          id: 'public_moderator_dashboard',
          severity: 'recommended',
          state: 'ready',
        }),
      ]),
    )
  })

  test('creates a topic and first post in void with public-safe body readback', async () => {
    const store = new ForumRouteStore()
    const response = await route(store, '/api/forum/forums/void/topics', {
      body: {
        bodyText: 'This is a first test thread for the unlisted void forum.',
        title: 'Void test thread',
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'void-topic-create-1',
      },
      method: 'POST',
    })
    const body = (await response.json()) as {
      topicHref: string
      webUrl: string
      topic: { topicId: string }
    }

    expect(response.status).toBe(201)
    expect(body).toMatchObject({
      firstPost: {
        bodyText: 'This is a first test thread for the unlisted void forum.',
        postNumber: 1,
      },
      idempotent: false,
      topic: {
        postCount: 1,
        slug: 'void-test-thread',
        title: 'Void test thread',
      },
    })
    expect(body.topicHref).toBe(`/forum/t/${body.topic.topicId}`)
    expect(body.webUrl).toBe(`/forum/t/${body.topic.topicId}`)
    expect(store.forums[1]?.topic_count).toBe(1)
    expect(store.forums[1]?.post_count).toBe(1)
  })

  test('creates listed-forum topics from an unclaimed agent token', async () => {
    const store = new ForumRouteStore()
    const response = await route(
      store,
      '/api/forum/forums/site-builder-help/topics',
      {
        agentClaimed: false,
        body: {
          bodyText: 'This unclaimed agent can publish open-forum speech.',
          title: 'Unclaimed listed thread',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'listed-topic-unclaimed-1',
        },
        method: 'POST',
      },
    )

    expect(response.status).toBe(201)
    await expect(response.json()).resolves.toMatchObject({
      firstPost: {
        bodyText: 'This unclaimed agent can publish open-forum speech.',
        postNumber: 1,
      },
      idempotent: false,
      topic: { postCount: 1, title: 'Unclaimed listed thread' },
    })
    expect(store.forums[0]?.topic_count).toBe(2)
  })

  test('creates listed-forum topics and replies with a claimed agent token', async () => {
    const store = new ForumRouteStore()
    const topicResponse = await route(
      store,
      '/api/forum/forums/site-builder-help/topics',
      {
        body: {
          bodyText: 'This is a scoped listed-forum thread.',
          title: 'Scoped listed thread',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'listed-topic-create-1',
        },
        method: 'POST',
      },
    )
    const topicBody = (await topicResponse.json()) as Readonly<{
      topic: Readonly<{ topicId: string }>
    }>
    const replyResponse = await route(
      store,
      `/api/forum/topics/${topicBody.topic.topicId}/posts`,
      {
        body: { bodyText: 'This is a scoped listed-forum reply.' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'listed-reply-create-1',
        },
        method: 'POST',
      },
    )

    expect(topicResponse.status).toBe(201)
    expect(replyResponse.status).toBe(201)
    await expect(replyResponse.json()).resolves.toMatchObject({
      idempotent: false,
      post: { bodyText: 'This is a scoped listed-forum reply.', postNumber: 2 },
      topic: { postCount: 2, title: 'Scoped listed thread' },
    })
    expect(store.forums[0]?.topic_count).toBe(2)
    expect(store.forums[0]?.post_count).toBe(3)
  })

  test('creates listed-forum replies from an unclaimed agent token', async () => {
    const store = new ForumRouteStore()
    const topicResponse = await route(
      store,
      '/api/forum/forums/site-builder-help/topics',
      {
        body: {
          bodyText: 'Claimed agent creates the topic.',
          title: 'Claimed starter thread',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'listed-topic-claimed-before-unclaimed-reply',
        },
        method: 'POST',
      },
    )
    const topicBody = (await topicResponse.json()) as Readonly<{
      topic: Readonly<{ topicId: string }>
    }>
    const replyResponse = await route(
      store,
      `/api/forum/topics/${topicBody.topic.topicId}/posts`,
      {
        agentClaimed: false,
        body: { bodyText: 'Unclaimed public reply.' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'listed-reply-unclaimed-1',
        },
        method: 'POST',
      },
    )

    expect(replyResponse.status).toBe(201)
    await expect(replyResponse.json()).resolves.toMatchObject({
      idempotent: false,
      post: { bodyText: 'Unclaimed public reply.', postNumber: 2 },
      topic: { postCount: 2 },
    })
    expect(store.forums[0]?.post_count).toBe(3)
  })

  test('rate-limits excessive agent topic creation without revoking posting authority', async () => {
    const store = new ForumRouteStore()
    const initialTopicCount = store.topics.length
    const recentTopicPosts = Array.from({ length: 3 }, (_, index) => ({
      actor_json: authenticatedAgentActorJson,
      archived_at: null,
      body_text: `Recent topic body ${index}.`,
      content_ref: `content.forum.route_test.recent_topic.${index}`,
      created_at: `2026-05-28T20:2${index}:00.000Z`,
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: `aaaaaaaa-3333-4333-8333-00000000000${index}`,
      idempotency_key: `recent-topic-post-${index}`,
      parent_post_id: null,
      post_number: 1,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible' as const,
      topic_id: `aaaaaaaa-3333-4333-8333-00000000010${index}`,
      updated_at: `2026-05-28T20:2${index}:00.000Z`,
    }))
    store.posts.push(...recentTopicPosts)

    const response = await route(
      store,
      '/api/forum/forums/site-builder-help/topics',
      {
        body: {
          bodyText: 'A fourth topic body inside the agent flood window.',
          title: 'Fourth topic attempt',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'listed-topic-rate-limited',
        },
        method: 'POST',
      },
    )

    expect(response.status).toBe(429)
    expect(response.headers.get('RateLimit-Limit')).toBe('3')
    expect(response.headers.get('X-OpenAgents-Paid-Recovery')).toBe('wait_only')
    await expect(response.json()).resolves.toMatchObject({
      actionKind: 'topic',
      error: 'forum_rate_limited',
      paidRecovery: 'wait_only',
      recoveryModes: ['wait', 'operator_review'],
    })
    expect(store.topics).toHaveLength(initialTopicCount)
  })

  test('rate-limits excessive agent replies and rejects recent duplicate bodies', async () => {
    const store = new ForumRouteStore()
    const recentReplies = Array.from({ length: 12 }, (_, index) => ({
      actor_json: authenticatedAgentActorJson,
      archived_at: null,
      body_text: `Recent reply body ${index}.`,
      content_ref: `content.forum.route_test.recent_reply.${index}`,
      created_at: `2026-05-28T20:25:${String(index).padStart(2, '0')}.000Z`,
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: `aaaaaaaa-4444-4444-8444-0000000000${String(index).padStart(2, '0')}`,
      idempotency_key: `recent-reply-post-${index}`,
      parent_post_id: '66666666-6666-4666-8666-666666666666',
      post_number: index + 2,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible' as const,
      topic_id: '55555555-5555-4555-8555-555555555555',
      updated_at: `2026-05-28T20:25:${String(index).padStart(2, '0')}.000Z`,
    }))
    store.posts.push(...recentReplies)

    const limitedResponse = await route(
      store,
      '/api/forum/topics/55555555-5555-4555-8555-555555555555/posts',
      {
        body: { bodyText: 'Thirteenth reply inside the flood window.' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'listed-reply-rate-limited',
        },
        method: 'POST',
      },
    )

    expect(limitedResponse.status).toBe(429)
    expect(limitedResponse.headers.get('RateLimit-Limit')).toBe('12')
    await expect(limitedResponse.json()).resolves.toMatchObject({
      actionKind: 'reply',
      error: 'forum_rate_limited',
      paidRecovery: 'wait_only',
    })

    const duplicateStore = new ForumRouteStore()
    duplicateStore.posts.push({
      actor_json: authenticatedAgentActorJson,
      archived_at: null,
      body_text: 'Duplicate body that should only appear once in the window.',
      content_ref: 'content.forum.route_test.duplicate',
      created_at: '2026-05-28T20:25:00.000Z',
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: 'aaaaaaaa-5555-4555-8555-000000000001',
      idempotency_key: 'recent-duplicate-post',
      parent_post_id: '66666666-6666-4666-8666-666666666666',
      post_number: 2,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'visible',
      topic_id: '55555555-5555-4555-8555-555555555555',
      updated_at: '2026-05-28T20:25:00.000Z',
    })
    const duplicateResponse = await route(
      duplicateStore,
      '/api/forum/topics/55555555-5555-4555-8555-555555555555/posts',
      {
        body: {
          bodyText:
            'Duplicate body that should only appear once in the window.',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'listed-reply-duplicate-body',
        },
        method: 'POST',
      },
    )

    expect(duplicateResponse.status).toBe(409)
    await expect(duplicateResponse.json()).resolves.toMatchObject({
      actionKind: 'reply',
      duplicateWindowSeconds: 1800,
      error: 'forum_duplicate_content',
      paidRecovery: 'wait_only',
    })
  })

  test('quotes, edits, reports, and tombstones owned posts without breaking chronology', async () => {
    const store = new ForumRouteStore()
    store.topics[0] = {
      ...store.topics[0]!,
      actor_json: authenticatedAgentActorJson,
    }
    store.posts[0] = {
      ...store.posts[0]!,
      actor_json: authenticatedAgentActorJson,
    }

    const postId = '66666666-6666-4666-8666-666666666666'
    const topicId = '55555555-5555-4555-8555-555555555555'
    const quoteResponse = await route(
      store,
      `/api/forum/topics/${topicId}/posts`,
      {
        body: {
          bodyText: 'Reply that quotes the seed post.',
          quotePostId: postId,
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'quote-owned-post-1',
        },
        method: 'POST',
      },
    )
    const editResponse = await route(store, `/api/forum/posts/${postId}`, {
      body: { bodyText: 'Edited public-safe seed body.' },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'edit-owned-post-1',
      },
      method: 'PATCH',
    })
    const reportResponse = await route(
      store,
      `/api/forum/posts/${postId}/reports`,
      {
        body: { reason: 'off_topic' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'report-owned-post-1',
        },
        method: 'POST',
      },
    )
    const reportRetryResponse = await route(
      store,
      `/api/forum/posts/${postId}/reports`,
      {
        body: { reason: 'off_topic' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'report-owned-post-1',
        },
        method: 'POST',
      },
    )
    const deleteResponse = await route(store, `/api/forum/posts/${postId}`, {
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'tombstone-owned-post-1',
      },
      method: 'DELETE',
    })
    const topicResponse = await route(store, `/api/forum/topics/${topicId}`)

    expect(quoteResponse.status).toBe(201)
    await expect(quoteResponse.json()).resolves.toMatchObject({
      post: {
        bodyText: 'Reply that quotes the seed post.',
        postNumber: 2,
        quotePostId: postId,
      },
    })
    expect(editResponse.status).toBe(200)
    await expect(editResponse.json()).resolves.toMatchObject({
      action: 'edit',
      idempotent: false,
      post: {
        bodyText: 'Edited public-safe seed body.',
        state: 'edited',
      },
    })
    expect(reportResponse.status).toBe(201)
    await expect(reportResponse.json()).resolves.toMatchObject({
      idempotent: false,
      report: {
        reason: 'off_topic',
        status: 'open',
        targetId: postId,
        targetKind: 'post',
      },
    })
    expect(reportRetryResponse.status).toBe(200)
    await expect(reportRetryResponse.json()).resolves.toMatchObject({
      idempotent: true,
      report: {
        reason: 'off_topic',
        targetId: postId,
      },
    })
    expect(deleteResponse.status).toBe(200)
    await expect(deleteResponse.json()).resolves.toMatchObject({
      action: 'tombstone',
      post: {
        bodyText: null,
        postNumber: 1,
        state: 'tombstoned',
      },
    })
    // A deleted (tombstoned) post must NOT appear in the thread at all: no
    // placeholder, no empty shell. Only the surviving visible reply remains,
    // and the topic counts drop to reflect the single live post.
    const topicBody = (await topicResponse.json()) as {
      posts: ReadonlyArray<{
        bodyText: string | null
        postNumber: number
        state: string
      }>
      topic: { postCount: number; replyCount: number }
    }
    expect(topicBody.posts).toHaveLength(1)
    expect(topicBody.posts).toEqual([
      expect.objectContaining({
        bodyText: 'Reply that quotes the seed post.',
        postNumber: 2,
        state: 'visible',
      }),
    ])
    expect(
      topicBody.posts.some(post => post.state === 'tombstoned'),
    ).toBe(false)
    expect(topicBody.topic.postCount).toBe(1)
    expect(topicBody.topic.replyCount).toBe(0)
    expect(store.postRevisions.map(revision => revision.action_kind)).toEqual([
      'edit',
      'tombstone',
    ])
    expect(store.reports).toHaveLength(1)
  })

  test('deleting a parent post removes it from the thread without orphaning its surviving child reply', async () => {
    const store = new ForumRouteStore()
    store.topics[0] = {
      ...store.topics[0]!,
      actor_json: authenticatedAgentActorJson,
    }
    store.posts[0] = {
      ...store.posts[0]!,
      actor_json: authenticatedAgentActorJson,
    }

    const parentPostId = '66666666-6666-4666-8666-666666666666'
    const topicId = '55555555-5555-4555-8555-555555555555'

    // A child reply threaded under the (soon-to-be-deleted) parent.
    const childResponse = await route(
      store,
      `/api/forum/topics/${topicId}/posts`,
      {
        body: {
          bodyText: 'Child reply under the parent post.',
          parentPostId,
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'thread-child-1',
        },
        method: 'POST',
      },
    )
    expect(childResponse.status).toBe(201)
    const childBody = (await childResponse.json()) as {
      post: { parentPostId: string | null; postId: string; postNumber: number }
    }
    expect(childBody.post.parentPostId).toBe(parentPostId)
    const childPostId = childBody.post.postId

    // Author deletes the PARENT.
    const deleteResponse = await route(
      store,
      `/api/forum/posts/${parentPostId}`,
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'thread-delete-parent-1',
        },
        method: 'DELETE',
      },
    )
    expect(deleteResponse.status).toBe(200)
    await expect(deleteResponse.json()).resolves.toMatchObject({
      action: 'tombstone',
      post: { state: 'tombstoned' },
    })

    // Idempotent repeat with the SAME key returns the prior result, not a
    // second tombstone (counts must not be decremented twice).
    const idempotentRepeat = await route(
      store,
      `/api/forum/posts/${parentPostId}`,
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'thread-delete-parent-1',
        },
        method: 'DELETE',
      },
    )
    expect(idempotentRepeat.status).toBe(200)
    await expect(idempotentRepeat.json()).resolves.toMatchObject({
      idempotent: true,
      post: { state: 'tombstoned' },
    })

    const topicResponse = await route(store, `/api/forum/topics/${topicId}`)
    const topicBody = (await topicResponse.json()) as {
      posts: ReadonlyArray<{
        bodyText: string | null
        contentRef: string
        parentPostId: string | null
        postId: string
        state: string
      }>
      topic: { postCount: number; replyCount: number }
    }

    // The deleted parent is GONE from the thread (no placeholder, no shell).
    expect(topicBody.posts.map(post => post.postId)).toEqual([childPostId])
    expect(
      topicBody.posts.some(post => post.postId === parentPostId),
    ).toBe(false)
    // No post renders the unresolved `content.forum.post.<id>` placeholder.
    expect(topicBody.posts.some(post => post.bodyText === null)).toBe(false)
    // The surviving child still carries its parent ref (no orphan/crash) and
    // its own body, and counts reflect exactly one live post.
    expect(topicBody.posts[0]?.parentPostId).toBe(parentPostId)
    expect(topicBody.posts[0]?.bodyText).toBe(
      'Child reply under the parent post.',
    )
    expect(topicBody.topic.postCount).toBe(1)
    expect(topicBody.topic.replyCount).toBe(0)
  })

  test('forum post deletion is author-only and returns typed 403/404 errors', async () => {
    const postId = '66666666-6666-4666-8666-666666666666'

    // Non-author cannot delete: the seed post is NOT owned by the route-test
    // agent in a default store, so a DELETE is forbidden.
    const foreignStore = new ForumRouteStore()
    const forbiddenResponse = await route(
      foreignStore,
      `/api/forum/posts/${postId}`,
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'delete-foreign-post-1',
        },
        method: 'DELETE',
      },
    )
    expect(forbiddenResponse.status).toBe(403)
    await expect(forbiddenResponse.json()).resolves.toMatchObject({
      error: 'forbidden',
      reason: 'only the post author can tombstone this post',
    })

    // Missing post: 404.
    const missingStore = new ForumRouteStore()
    missingStore.topics[0] = {
      ...missingStore.topics[0]!,
      actor_json: authenticatedAgentActorJson,
    }
    missingStore.posts[0] = {
      ...missingStore.posts[0]!,
      actor_json: authenticatedAgentActorJson,
    }
    const missingResponse = await route(
      missingStore,
      '/api/forum/posts/00000000-0000-4000-8000-000000000000',
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'delete-missing-post-1',
        },
        method: 'DELETE',
      },
    )
    expect(missingResponse.status).toBe(404)
    await expect(missingResponse.json()).resolves.toMatchObject({
      error: 'not_found',
    })

    // Missing Idempotency-Key: 400.
    const noKeyResponse = await route(
      missingStore,
      `/api/forum/posts/${postId}`,
      {
        headers: { authorization: 'Bearer oa_agent_route_test' },
        method: 'DELETE',
      },
    )
    expect(noKeyResponse.status).toBe(400)
    await expect(noKeyResponse.json()).resolves.toMatchObject({
      error: 'bad_request',
      reason: 'Idempotency-Key header is required',
    })
  })

  test('renames owned topics and refuses non-author, missing, or invalid renames', async () => {
    const topicId = '55555555-5555-4555-8555-555555555555'

    const ownedStore = new ForumRouteStore()
    ownedStore.topics[0] = {
      ...ownedStore.topics[0]!,
      actor_json: authenticatedAgentActorJson,
    }

    const renameResponse = await route(
      ownedStore,
      `/api/forum/topics/${topicId}`,
      {
        body: { title: 'Renamed by the topic author' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'rename-owned-topic-1',
        },
        method: 'PATCH',
      },
    )

    expect(renameResponse.status).toBe(200)
    await expect(renameResponse.json()).resolves.toMatchObject({
      action: 'rename',
      idempotent: false,
      topic: { title: 'Renamed by the topic author' },
    })

    const renamedTopicResponse = await route(
      ownedStore,
      `/api/forum/topics/${topicId}`,
    )
    await expect(renamedTopicResponse.json()).resolves.toMatchObject({
      topic: { title: 'Renamed by the topic author' },
    })

    const shortTitleResponse = await route(
      ownedStore,
      `/api/forum/topics/${topicId}`,
      {
        body: { title: 'ab' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'rename-owned-topic-short-1',
        },
        method: 'PATCH',
      },
    )

    expect(shortTitleResponse.status).toBe(400)
    await expect(shortTitleResponse.json()).resolves.toMatchObject({
      error: 'bad_request',
    })

    const longTitleResponse = await route(
      ownedStore,
      `/api/forum/topics/${topicId}`,
      {
        body: { title: 'x'.repeat(161) },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'rename-owned-topic-long-1',
        },
        method: 'PATCH',
      },
    )

    expect(longTitleResponse.status).toBe(400)
    await expect(longTitleResponse.json()).resolves.toMatchObject({
      error: 'bad_request',
    })

    const missingIdempotencyResponse = await route(
      ownedStore,
      `/api/forum/topics/${topicId}`,
      {
        body: { title: 'Rename without an idempotency key' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
        },
        method: 'PATCH',
      },
    )

    expect(missingIdempotencyResponse.status).toBe(400)
    await expect(missingIdempotencyResponse.json()).resolves.toMatchObject({
      error: 'bad_request',
      reason: 'Idempotency-Key header is required',
    })

    const foreignStore = new ForumRouteStore()
    const forbiddenResponse = await route(
      foreignStore,
      `/api/forum/topics/${topicId}`,
      {
        body: { title: 'Rename attempt by a non-author' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'rename-foreign-topic-1',
        },
        method: 'PATCH',
      },
    )

    expect(forbiddenResponse.status).toBe(403)
    await expect(forbiddenResponse.json()).resolves.toMatchObject({
      error: 'forbidden',
      reason: 'only the topic author can rename this topic',
    })

    const missingStore = new ForumRouteStore()
    missingStore.topics[0] = {
      ...missingStore.topics[0]!,
      actor_json: authenticatedAgentActorJson,
    }
    const missingResponse = await route(
      missingStore,
      '/api/forum/topics/00000000-0000-4000-8000-000000000000',
      {
        body: { title: 'Rename a topic that does not exist' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'rename-missing-topic-1',
        },
        method: 'PATCH',
      },
    )

    expect(missingResponse.status).toBe(404)
    await expect(missingResponse.json()).resolves.toMatchObject({
      error: 'not_found',
    })
  })

  test('validates reply parentPostId against same-topic visible posts', async () => {
    const store = new ForumRouteStore()
    const topicId = '55555555-5555-4555-8555-555555555555'
    const seedPostId = '66666666-6666-4666-8666-666666666666'
    const crossTopicPostId = '88888888-5001-4001-8001-888888888888'

    const validResponse = await route(
      store,
      `/api/forum/topics/${topicId}/posts`,
      {
        body: {
          bodyText: 'Reply threaded under the seed post.',
          parentPostId: seedPostId,
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'parent-ref-valid-1',
        },
        method: 'POST',
      },
    )
    const truncatedResponse = await route(
      store,
      `/api/forum/topics/${topicId}/posts`,
      {
        body: {
          bodyText: 'Reply carrying a truncated parent ref.',
          parentPostId: '95993529',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'parent-ref-truncated-1',
        },
        method: 'POST',
      },
    )
    const crossTopicResponse = await route(
      store,
      `/api/forum/topics/${topicId}/posts`,
      {
        body: {
          bodyText: 'Reply carrying a cross-topic parent ref.',
          parentPostId: crossTopicPostId,
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'parent-ref-cross-topic-1',
        },
        method: 'POST',
      },
    )

    const seedIndex = store.posts.findIndex(item => item.id === seedPostId)
    store.posts[seedIndex] = {
      ...store.posts[seedIndex]!,
      state: 'tombstoned',
    }

    const tombstonedResponse = await route(
      store,
      `/api/forum/topics/${topicId}/posts`,
      {
        body: {
          bodyText: 'Reply carrying a tombstoned parent ref.',
          parentPostId: seedPostId,
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'parent-ref-tombstoned-1',
        },
        method: 'POST',
      },
    )

    expect(validResponse.status).toBe(201)
    await expect(validResponse.json()).resolves.toMatchObject({
      post: { parentPostId: seedPostId, postNumber: 2 },
    })
    expect(truncatedResponse.status).toBe(400)
    await expect(truncatedResponse.json()).resolves.toMatchObject({
      error: 'bad_request',
      reason: 'parentPostId must reference an existing post',
    })
    expect(crossTopicResponse.status).toBe(400)
    await expect(crossTopicResponse.json()).resolves.toMatchObject({
      error: 'bad_request',
      reason: 'parentPostId must belong to the target topic',
    })
    expect(tombstonedResponse.status).toBe(400)
    await expect(tombstonedResponse.json()).resolves.toMatchObject({
      error: 'bad_request',
      reason: 'parentPostId must reference a visible post',
    })
    expect(
      store.posts.filter(item => item.topic_id === topicId),
    ).toHaveLength(2)
  })

  test('honors PATCH parentPostId repairs with validation and a cycle guard', async () => {
    const store = new ForumRouteStore()
    const topicId = '55555555-5555-4555-8555-555555555555'
    const seedPostId = '66666666-6666-4666-8666-666666666666'

    const firstReplyResponse = await route(
      store,
      `/api/forum/topics/${topicId}/posts`,
      {
        body: { bodyText: 'First reply in the repair thread.' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'parent-repair-reply-1',
        },
        method: 'POST',
      },
    )
    const firstReply = (await firstReplyResponse.json()) as Readonly<{
      post: Readonly<{ postId: string }>
    }>
    const secondReplyResponse = await route(
      store,
      `/api/forum/topics/${topicId}/posts`,
      {
        body: {
          bodyText: 'Second reply nested under the first.',
          parentPostId: firstReply.post.postId,
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'parent-repair-reply-2',
        },
        method: 'POST',
      },
    )
    const secondReply = (await secondReplyResponse.json()) as Readonly<{
      post: Readonly<{ postId: string }>
    }>

    const cycleResponse = await route(
      store,
      `/api/forum/posts/${firstReply.post.postId}`,
      {
        body: {
          bodyText: 'Attempt to nest the first reply under its child.',
          parentPostId: secondReply.post.postId,
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'parent-repair-cycle-1',
        },
        method: 'PATCH',
      },
    )
    const selfResponse = await route(
      store,
      `/api/forum/posts/${secondReply.post.postId}`,
      {
        body: {
          bodyText: 'Attempt to nest the second reply under itself.',
          parentPostId: secondReply.post.postId,
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'parent-repair-self-1',
        },
        method: 'PATCH',
      },
    )

    // Mirror the live dangling-ref shape: a truncated parent ref persisted
    // before validation existed (#4856).
    const brokenIndex = store.posts.findIndex(
      item => item.id === secondReply.post.postId,
    )
    store.posts[brokenIndex] = {
      ...store.posts[brokenIndex]!,
      parent_post_id: '95993529',
    }

    const missingResponse = await route(
      store,
      `/api/forum/posts/${secondReply.post.postId}`,
      {
        body: {
          bodyText: 'Attempt to repair with another dangling ref.',
          parentPostId: '12345678',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'parent-repair-missing-1',
        },
        method: 'PATCH',
      },
    )
    const repairResponse = await route(
      store,
      `/api/forum/posts/${secondReply.post.postId}`,
      {
        body: {
          bodyText: 'Repaired thread ref pointing at the seed post.',
          parentPostId: seedPostId,
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'parent-repair-fix-1',
        },
        method: 'PATCH',
      },
    )
    const repairRetryResponse = await route(
      store,
      `/api/forum/posts/${secondReply.post.postId}`,
      {
        body: {
          bodyText: 'Repaired thread ref pointing at the seed post.',
          parentPostId: seedPostId,
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'parent-repair-fix-1',
        },
        method: 'PATCH',
      },
    )
    const topLevelResponse = await route(
      store,
      `/api/forum/posts/${firstReply.post.postId}`,
      {
        body: {
          bodyText: 'First reply re-parented to top level.',
          parentPostId: null,
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'parent-repair-top-level-1',
        },
        method: 'PATCH',
      },
    )

    expect(cycleResponse.status).toBe(400)
    await expect(cycleResponse.json()).resolves.toMatchObject({
      error: 'bad_request',
      reason: 'parentPostId must not create a reply cycle',
    })
    expect(selfResponse.status).toBe(400)
    await expect(selfResponse.json()).resolves.toMatchObject({
      error: 'bad_request',
      reason: 'parentPostId must not reference the edited post',
    })
    expect(missingResponse.status).toBe(400)
    await expect(missingResponse.json()).resolves.toMatchObject({
      error: 'bad_request',
      reason: 'parentPostId must reference an existing post',
    })
    expect(repairResponse.status).toBe(200)
    await expect(repairResponse.json()).resolves.toMatchObject({
      action: 'edit',
      idempotent: false,
      post: {
        bodyText: 'Repaired thread ref pointing at the seed post.',
        parentPostId: seedPostId,
        state: 'edited',
      },
    })
    expect(repairRetryResponse.status).toBe(200)
    await expect(repairRetryResponse.json()).resolves.toMatchObject({
      idempotent: true,
      post: { parentPostId: seedPostId },
    })
    expect(topLevelResponse.status).toBe(200)
    await expect(topLevelResponse.json()).resolves.toMatchObject({
      action: 'edit',
      post: { parentPostId: null },
    })
    expect(store.postRevisions.map(revision => revision.action_kind)).toEqual([
      'edit',
      'edit',
    ])
  })

  test('moderator queue and actions are admin-only and public-safe', async () => {
    const store = new ForumRouteStore()
    const topicId = '55555555-5555-4555-8555-555555555555'
    const postId = '66666666-6666-4666-8666-666666666666'
    const reportId = 'aaaaaaaa-2222-4222-8222-000000000001'

    store.reports.push({
      archived_at: null,
      created_at: '2026-06-05T20:11:00.000Z',
      id: reportId,
      idempotency_key: 'moderation-report-seed',
      public_projection_json: projectionJson,
      reason_ref: 'forum.report.reason.spam',
      reporter_actor_ref: 'agent:reporter',
      status: 'open',
      target_id: postId,
      target_kind: 'post',
      updated_at: '2026-06-05T20:11:00.000Z',
    })
    store.posts.push({
      actor_json: actorJson,
      archived_at: null,
      body_text: 'Held for review body.',
      content_ref: 'content.forum.route_test.held',
      created_at: '2026-06-05T20:12:00.000Z',
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: 'aaaaaaaa-2222-4222-8222-000000000002',
      idempotency_key: 'held-post',
      parent_post_id: null,
      post_number: 2,
      public_projection_json: projectionJson,
      quote_post_id: null,
      receipt_refs_json: '[]',
      revision_ref: null,
      state: 'held_for_review',
      topic_id: topicId,
      updated_at: '2026-06-05T20:12:00.000Z',
    })
    store.topics.push({
      actor_json: actorJson,
      archived_at: null,
      created_at: '2026-06-05T20:13:00.000Z',
      first_post_id: 'aaaaaaaa-2222-4222-8222-000000000003',
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: 'aaaaaaaa-2222-4222-8222-000000000004',
      idempotency_key: 'hidden-topic',
      latest_post_id: 'aaaaaaaa-2222-4222-8222-000000000003',
      pin_state: 'normal',
      post_count: 1,
      public_projection_json: projectionJson,
      score_ref: null,
      slug: 'hidden-topic',
      state: 'hidden',
      title: 'Hidden topic',
      updated_at: '2026-06-05T20:13:00.000Z',
    })

    const unauthenticated = await route(store, '/api/forum/moderation/queue')
    const bearerOnly = await route(store, '/api/forum/moderation/queue', {
      headers: { authorization: 'Bearer oa_agent_route_test' },
    })
    const nonAdmin = await route(store, '/api/forum/moderation/queue', {
      moderator: 'non_admin',
    })
    const queueResponse = await route(store, '/api/forum/moderation/queue', {
      moderator: 'admin',
    })
    const reportResponse = await route(
      store,
      `/api/forum/moderation/reports/${reportId}`,
      { moderator: 'admin' },
    )
    const hidePostResponse = await route(
      store,
      `/api/forum/moderation/posts/${postId}/hide`,
      {
        body: { reason: 'spam' },
        headers: { 'idempotency-key': 'moderator-hide-post-1' },
        method: 'POST',
        moderator: 'admin',
      },
    )
    const hidePostRetryResponse = await route(
      store,
      `/api/forum/moderation/posts/${postId}/hide`,
      {
        body: { reason: 'spam' },
        headers: { 'idempotency-key': 'moderator-hide-post-1' },
        method: 'POST',
        moderator: 'admin',
      },
    )
    const publicHiddenPost = await route(store, `/api/forum/posts/${postId}`)
    const lockTopicResponse = await route(
      store,
      `/api/forum/moderation/topics/${topicId}/lock`,
      {
        headers: { 'idempotency-key': 'moderator-lock-topic-1' },
        method: 'POST',
        moderator: 'admin',
      },
    )
    const reviewedReportResponse = await route(
      store,
      `/api/forum/moderation/reports/${reportId}/mark-reviewed`,
      {
        headers: { 'idempotency-key': 'moderator-review-report-1' },
        method: 'POST',
        moderator: 'admin',
      },
    )

    expect(unauthenticated.status).toBe(401)
    expect(bearerOnly.status).toBe(401)
    expect(nonAdmin.status).toBe(403)
    expect(queueResponse.status).toBe(200)
    await expect(queueResponse.json()).resolves.toMatchObject({
      items: expect.arrayContaining([
        expect.objectContaining({
          itemKind: 'report',
          reportId,
          reporterActorRef: 'agent:reporter',
          targetId: postId,
        }),
        expect.objectContaining({
          itemKind: 'post_review',
          targetState: 'held_for_review',
        }),
        expect.objectContaining({
          itemKind: 'topic_review',
          targetState: 'hidden',
        }),
      ]),
    })
    await expect(reportResponse.json()).resolves.toMatchObject({
      bodyText: 'Seed route-test body.',
      report: {
        id: reportId,
        reporter_actor_ref: 'agent:reporter',
      },
    })
    expect(hidePostResponse.status).toBe(201)
    await expect(hidePostResponse.json()).resolves.toMatchObject({
      idempotent: false,
      moderationEvent: {
        actionKind: 'moderator_hide_post',
        targetId: postId,
        targetKind: 'post',
      },
      target: { state: 'hidden' },
    })
    expect(hidePostRetryResponse.status).toBe(200)
    await expect(hidePostRetryResponse.json()).resolves.toMatchObject({
      idempotent: true,
      moderationEvent: {
        actionKind: 'moderator_hide_post',
        targetId: postId,
      },
    })
    expect(publicHiddenPost.status).toBe(404)
    expect(lockTopicResponse.status).toBe(201)
    await expect(lockTopicResponse.json()).resolves.toMatchObject({
      moderationEvent: {
        actionKind: 'moderator_lock_topic',
        targetId: topicId,
      },
      target: { state: 'locked' },
    })
    expect(reviewedReportResponse.status).toBe(201)
    await expect(reviewedReportResponse.json()).resolves.toMatchObject({
      moderationEvent: {
        actionKind: 'moderator_mark_reviewed_report',
        reportId,
        targetKind: 'report',
      },
      target: { status: 'resolved' },
    })
    expect(store.reports[0]?.status).toBe('resolved')
    expect(store.moderationEvents).toHaveLength(3)
  })

  test('keeps redemption writes atomic when a mid-batch statement fails', async () => {
    const store = new ForumRouteStore()
    const path = '/api/forum/orange-check'
    const previewResponse = await route(
      store,
      '/api/forum/paid-actions/preview',
      {
        body: {
          actionKind: 'orange_check',
          method: 'POST',
          path,
          requestBodyDigest: 'sha256:orange-check-atomicity-body',
          routeParams: {},
          spendCap: { amount: 500, asset: 'usd' },
          target: { forumId: null, postId: null, topicId: null },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'orange-check-atomic-preview-1',
        },
        method: 'POST',
      },
    )
    const preview = (await previewResponse.json()) as Readonly<{
      challenge: Readonly<{ challengeId: string }>
    }>
    const privatePaymentResponse = await route(
      store,
      '/api/forum/paid-actions/private-payment',
      {
        body: {
          challengeId: preview.challenge.challengeId,
          method: 'POST',
          path,
          requestBodyDigest: 'sha256:orange-check-atomicity-body',
          routeParams: {},
          spendCap: { amount: 500, asset: 'usd' },
        },
        headers: { authorization: 'Bearer oa_agent_route_test' },
        method: 'POST',
      },
    )
    const privatePayment = (await privatePaymentResponse.json()) as Readonly<{
      privatePayment: Readonly<{ credential: string; l402ProofRef: string }>
    }>
    const baseClient = forumHostedMdkClient()
    const paidClient = {
      ...baseClient,
      getCheckoutStatus: (
        request: Parameters<typeof baseClient.getCheckoutStatus>[0],
      ) =>
        Effect.map(baseClient.getCheckoutStatus(request), status => ({
          ...status,
          status: 'payment_received' as const,
        })),
    }
    const redeemRequest = (idempotencyKey: string) =>
      ({
        body: {
          challengeId: preview.challenge.challengeId,
          l402ProofRef: privatePayment.privatePayment.l402ProofRef,
          method: 'POST',
          path,
          requestBodyDigest: 'sha256:orange-check-atomicity-body',
          routeParams: {},
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': idempotencyKey,
          'x-openagents-l402': `${privatePayment.privatePayment.credential}:${privatePayment.privatePayment.l402ProofRef}`,
        },
        hostedMdkClient: paidClient,
        method: 'POST' as const,
      }) as const

    store.failInsertsInto = 'forum_l402_redemptions'
    const failed = await route(
      store,
      '/api/forum/paid-actions/redeem',
      redeemRequest('orange-check-atomic-redeem-fail'),
    )

    store.failInsertsInto = null
    const retried = await route(
      store,
      '/api/forum/paid-actions/redeem',
      redeemRequest('orange-check-atomic-redeem-retry'),
    )
    const retriedBody = (await retried.json()) as Readonly<{
      orangeCheck: Readonly<{ active: boolean }>
      replayed: boolean
    }>

    expect(failed.status).toBe(500)
    expect(retried.status).toBe(201)
    expect(retriedBody.replayed).toBe(false)
    expect(retriedBody.orangeCheck).toMatchObject({ active: true })
    expect(store.redemptions).toHaveLength(1)
    expect(
      store.receipts.filter(row => row.action_kind === 'orange_check'),
    ).toHaveLength(1)
  })

  test('sells the orange check through preview, private payment, and redeem with entitlement fulfillment', async () => {
    const store = new ForumRouteStore()
    const path = '/api/forum/orange-check'
    const previewResponse = await route(
      store,
      '/api/forum/paid-actions/preview',
      {
        body: {
          actionKind: 'orange_check',
          method: 'POST',
          path,
          requestBodyDigest: 'sha256:orange-check-purchase-body',
          routeParams: {},
          spendCap: { amount: 500, asset: 'usd' },
          target: { forumId: null, postId: null, topicId: null },
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'orange-check-preview-1',
        },
        method: 'POST',
      },
    )
    const preview = (await previewResponse.json()) as Readonly<{
      challenge: Readonly<{ challengeId: string }>
    }>
    const privatePaymentResponse = await route(
      store,
      '/api/forum/paid-actions/private-payment',
      {
        body: {
          challengeId: preview.challenge.challengeId,
          method: 'POST',
          path,
          requestBodyDigest: 'sha256:orange-check-purchase-body',
          routeParams: {},
          spendCap: { amount: 500, asset: 'usd' },
        },
        headers: { authorization: 'Bearer oa_agent_route_test' },
        method: 'POST',
      },
    )
    const privatePayment = (await privatePaymentResponse.json()) as Readonly<{
      privatePayment: Readonly<{ credential: string; l402ProofRef: string }>
    }>
    const unpaidRedeem = await route(
      store,
      '/api/forum/paid-actions/redeem',
      {
        body: {
          challengeId: preview.challenge.challengeId,
          l402ProofRef: privatePayment.privatePayment.l402ProofRef,
          method: 'POST',
          path,
          requestBodyDigest: 'sha256:orange-check-purchase-body',
          routeParams: {},
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'orange-check-redeem-unpaid',
          'x-openagents-l402': `${privatePayment.privatePayment.credential}:${privatePayment.privatePayment.l402ProofRef}`,
        },
        method: 'POST',
      },
    )
    const baseClient = forumHostedMdkClient()
    const paidClient = {
      ...baseClient,
      getCheckoutStatus: (
        request: Parameters<typeof baseClient.getCheckoutStatus>[0],
      ) =>
        Effect.map(baseClient.getCheckoutStatus(request), status => ({
          ...status,
          status: 'payment_received' as const,
        })),
    }
    const redeemResponse = await route(
      store,
      '/api/forum/paid-actions/redeem',
      {
        body: {
          challengeId: preview.challenge.challengeId,
          l402ProofRef: privatePayment.privatePayment.l402ProofRef,
          method: 'POST',
          path,
          requestBodyDigest: 'sha256:orange-check-purchase-body',
          routeParams: {},
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'orange-check-redeem-1',
          'x-openagents-l402': `${privatePayment.privatePayment.credential}:${privatePayment.privatePayment.l402ProofRef}`,
        },
        hostedMdkClient: paidClient,
        method: 'POST',
      },
    )
    const redeem = (await redeemResponse.json()) as Readonly<{
      orangeCheck: Readonly<{ active: boolean; badgeRef: string | null }>
    }>

    expect(previewResponse.status).toBe(200)
    expect(privatePaymentResponse.status).toBe(200)
    expect(unpaidRedeem.status).toBe(402)
    await expect(unpaidRedeem.json()).resolves.toMatchObject({
      error: 'orange_check_payment_not_received',
    })
    expect(redeemResponse.status).toBe(201)
    expect(redeem.orangeCheck).toMatchObject({ active: true })
    expect(store.orangeCheckEntitlements).toHaveLength(1)
    expect(store.orangeCheckEntitlements[0]).toMatchObject({
      actor_ref: expect.stringContaining('agent:'),
      paid_amount_cents: 500,
      state: 'active',
    })
    expect(JSON.stringify(redeem)).not.toMatch(/verified human|safe account/i)
    expect(JSON.stringify(redeem)).not.toContain('lntbs')
  })

  test('projects orange-check badges on profiles and posts from active entitlements', async () => {
    const store = new ForumRouteStore()
    const created = await route(
      store,
      '/api/forum/forums/site-builder-help/topics',
      {
        body: {
          bodyText: 'Orange badge projection test thread.',
          title: 'Orange badge thread',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'orange-topic-create-1',
        },
        method: 'POST',
      },
    )
    const createdBody = (await created.json()) as Readonly<{
      firstPost: Readonly<{ author: Readonly<{ actorRef: string }>; postId: string }>
    }>
    const actorRef = createdBody.firstPost.author.actorRef
    const postId = createdBody.firstPost.postId
    const before = await route(store, `/api/forum/posts/${postId}`)
    const beforeBody = (await before.json()) as Readonly<{
      authorOrangeCheck: Readonly<{ active: boolean }>
    }>

    store.orangeCheckEntitlements.push({
      action_ref: 'forum_money_action.test.orange',
      actor_ref: actorRef,
      agent_user_id: actorRef.replace('agent:', ''),
      created_at: '2026-06-09T00:00:00.000Z',
      id: 'orange_check_test_1',
      paid_amount_cents: 500,
      receipt_ref: 'orange_check_receipt.test_1',
      state: 'active',
      updated_at: '2026-06-09T00:00:00.000Z',
    })

    const after = await route(store, `/api/forum/posts/${postId}`)
    const afterBody = (await after.json()) as Readonly<{
      authorOrangeCheck: Readonly<{ active: boolean; badgeRef: string | null }>
    }>
    const profile = await route(
      store,
      `/api/forum/actors/${encodeURIComponent(actorRef)}/profile`,
    )
    const profileBody = (await profile.json()) as Readonly<{
      orangeCheck: Readonly<{ active: boolean; meaning: string }>
    }>

    expect(beforeBody.authorOrangeCheck).toMatchObject({ active: false })
    expect(afterBody.authorOrangeCheck).toMatchObject({
      active: true,
      badgeRef: 'orange_check_receipt.test_1',
    })
    expect(profile.status).toBe(200)
    expect(profileBody.orangeCheck).toMatchObject({ active: true })
    expect(profileBody.orangeCheck.meaning).toContain('owner-claimed')
    expect(JSON.stringify(afterBody)).not.toMatch(/verified human|safe account/i)
  })

  test('exports active orange-check entitlements as NIP-58 Nostr badge templates', async () => {
    const store = new ForumRouteStore()
    const actorRef = 'agent:orange-export-owner'
    const issuerPubkey = '11'.repeat(32)
    const recipientPubkey = '22'.repeat(32)
    store.orangeCheckEntitlements.push({
      action_ref: 'forum_money_action.test.orange_export',
      actor_ref: actorRef,
      agent_user_id: 'orange-export-owner',
      created_at: '2026-06-10T10:00:00.000Z',
      id: 'orange_check_export_1',
      paid_amount_cents: 500,
      receipt_ref: 'orange_check_receipt.export_1',
      state: 'active',
      updated_at: '2026-06-10T10:00:00.000Z',
    })

    const response = await route(
      store,
      `/api/forum/actors/${encodeURIComponent(actorRef)}/orange-check/nostr-export?recipientPubkey=${recipientPubkey}&issuerPubkey=${issuerPubkey}&relay=wss%3A%2F%2Frelay.openagents.example`,
    )
    const body = (await response.json()) as Readonly<{
      nostrExport: Readonly<{
        badgeAward: Readonly<{
          kind: number
          tags: ReadonlyArray<ReadonlyArray<string>>
        }>
        badgeDefinition: Readonly<{ kind: number }>
        badgeDefinitionAddress: string
        exportDigestRef: string
        receiptRef: string
      }>
    }>

    expect(response.status).toBe(200)
    expect(body.nostrExport).toMatchObject({
      badgeDefinitionAddress: `30009:${issuerPubkey}:openagents-orange-check`,
      exportDigestRef: expect.stringMatching(/^nostr_export\.orange_check\./),
      receiptRef: 'orange_check_receipt.export_1',
    })
    expect(body.nostrExport.badgeDefinition.kind).toBe(30009)
    expect(body.nostrExport.badgeAward.kind).toBe(8)
    expect(body.nostrExport.badgeAward.tags).toContainEqual([
      'p',
      recipientPubkey,
      'wss://relay.openagents.example',
    ])
    expect(JSON.stringify(body)).not.toMatch(/verified human|safe account/i)
    expect(JSON.stringify(body)).not.toMatch(/lnbc|preimage|mnemonic|wallet/i)
  })

  test('blocks orange-check Nostr export without entitlement or valid pubkeys', async () => {
    const store = new ForumRouteStore()
    const actorRef = 'agent:no-orange-export'
    const missing = await route(
      store,
      `/api/forum/actors/${encodeURIComponent(actorRef)}/orange-check/nostr-export?recipientPubkey=${'22'.repeat(32)}&issuerPubkey=${'11'.repeat(32)}`,
    )

    store.orangeCheckEntitlements.push({
      action_ref: 'forum_money_action.test.orange_export',
      actor_ref: actorRef,
      agent_user_id: 'no-orange-export',
      created_at: '2026-06-10T10:00:00.000Z',
      id: 'orange_check_export_2',
      paid_amount_cents: 500,
      receipt_ref: 'orange_check_receipt.export_2',
      state: 'active',
      updated_at: '2026-06-10T10:00:00.000Z',
    })

    const malformed = await route(
      store,
      `/api/forum/actors/${encodeURIComponent(actorRef)}/orange-check/nostr-export?recipientPubkey=bad&issuerPubkey=${'11'.repeat(32)}`,
    )
    const malformedBody = (await malformed.json()) as Readonly<{
      error: string
      reason: string
    }>

    expect(missing.status).toBe(404)
    expect(malformed.status).toBe(400)
    expect(malformedBody.error).toBe('bad_request')
    expect(malformedBody.reason).toContain('recipientPubkey')
  })

  test('pins and unpins topics through moderator actions with pinned-first ordering', async () => {
    const store = new ForumRouteStore()
    const created = await route(
      store,
      '/api/forum/forums/site-builder-help/topics',
      {
        body: {
          bodyText: 'This thread should be pinnable by moderators.',
          title: 'Pin candidate thread',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'pin-topic-create-1',
        },
        method: 'POST',
      },
    )
    const createdBody = (await created.json()) as Readonly<{
      topic: Readonly<{ topicId: string }>
    }>
    const topicId = createdBody.topic.topicId
    const nonModeratorPin = await route(
      store,
      `/api/forum/moderation/topics/${topicId}/pin`,
      {
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'pin-topic-agent-1',
        },
        method: 'POST',
      },
    )
    const pinResponse = await route(
      store,
      `/api/forum/moderation/topics/${topicId}/pin`,
      {
        headers: { 'idempotency-key': 'pin-topic-moderator-1' },
        method: 'POST',
        moderator: 'admin',
      },
    )
    const listAfterPin = await route(
      store,
      '/api/forum/forums/site-builder-help/topics',
    )
    const listBody = (await listAfterPin.json()) as Readonly<{
      topics: ReadonlyArray<Readonly<{ pinState: string; title: string }>>
    }>
    const unpinResponse = await route(
      store,
      `/api/forum/moderation/topics/${topicId}/unpin`,
      {
        headers: { 'idempotency-key': 'unpin-topic-moderator-1' },
        method: 'POST',
        moderator: 'admin',
      },
    )

    expect(created.status).toBe(201)
    expect(nonModeratorPin.status).toBe(401)
    expect(pinResponse.status).toBe(201)
    await expect(pinResponse.json()).resolves.toMatchObject({
      moderationEvent: {
        actionKind: 'moderator_pin_topic',
        targetId: topicId,
        targetKind: 'topic',
      },
      target: { pinState: 'sticky' },
    })
    expect(
      listBody.topics.find(topic => topic.title === 'Pin candidate thread'),
    ).toMatchObject({
      pinState: 'sticky',
    })
    expect(unpinResponse.status).toBe(201)
    await expect(unpinResponse.json()).resolves.toMatchObject({
      moderationEvent: {
        actionKind: 'moderator_unpin_topic',
        targetId: topicId,
      },
      target: { pinState: 'normal' },
    })
  })

  test('returns public-safe Site context activity and redacts private links', async () => {
    const store = new ForumRouteStore()
    const topicResponse = await route(
      store,
      '/api/forum/forums/site-builder-help/topics',
      {
        body: {
          bodyText: 'This thread is linked to a public Site revision.',
          context: {
            contextId: 'site_project_otec',
            contextKind: 'site',
            contextSlug: 'otec',
            contextTitle: 'OTEC Site',
            publicUrl: 'https://openagents.com/sites/otec',
            sourceRef: 'site_project:site_project_otec',
          },
          title: 'Context linked Site thread',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'listed-context-topic-create-1',
        },
        method: 'POST',
      },
    )
    const topicBody = (await topicResponse.json()) as Readonly<{
      topic: Readonly<{ topicId: string }>
    }>
    const publicActivity = await route(
      store,
      '/api/forum/contexts/site/site_project_otec/activity',
    )

    store.contextLinks.push({
      archived_at: null,
      context_id: 'site_project_private',
      context_kind: 'site',
      context_slug: 'private',
      context_title: 'private@example.com',
      created_at: '2026-06-05T20:00:00.000Z',
      forum_id: '33333333-3333-4333-8333-333333333333',
      id: 'private-context-link',
      post_id: null,
      public_projection_json: privateProjectionJson,
      public_url: 'https://openagents.com/sites/private?access_token=secret',
      source_ref: 'private_key:secret',
      target_id: topicBody.topic.topicId,
      target_kind: 'topic',
      topic_id: topicBody.topic.topicId,
    })

    const privateActivity = await route(
      store,
      '/api/forum/contexts/site/site_project_private/activity',
    )
    const privateBody = await privateActivity.text()

    expect(topicResponse.status).toBe(201)
    expect(publicActivity.status).toBe(200)
    await expect(publicActivity.json()).resolves.toMatchObject({
      context: {
        contextId: 'site_project_otec',
        contextKind: 'site',
      },
      contextLinks: [
        {
          contextTitle: 'OTEC Site',
          publicUrl: 'https://openagents.com/sites/otec',
        },
      ],
      posts: [{ bodyText: 'This thread is linked to a public Site revision.' }],
      topics: [{ title: 'Context linked Site thread' }],
    })
    expect(privateActivity.status).toBe(200)
    expect(JSON.parse(privateBody)).toMatchObject({
      contextLinks: [],
      posts: [],
      topics: [],
    })
    expect(privateBody).not.toContain('access_token')
    expect(privateBody).not.toContain('private@example.com')
  })

  test('accepts longform forum topic and reply bodies', async () => {
    const store = new ForumRouteStore()
    const longTopicBody = `Longform topic packet. ${'market evidence trap '.repeat(230)}end.`
    const longReplyBody = `Longform reply packet. ${'typed contribution proof '.repeat(230)}end.`
    const topicResponse = await route(
      store,
      '/api/forum/forums/site-builder-help/topics',
      {
        body: {
          bodyText: longTopicBody,
          title: 'Longform packet thread',
        },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'listed-longform-topic-create-1',
        },
        method: 'POST',
      },
    )
    const topicBody = (await topicResponse.json()) as Readonly<{
      topic: Readonly<{ topicId: string }>
    }>
    const replyResponse = await route(
      store,
      `/api/forum/topics/${topicBody.topic.topicId}/posts`,
      {
        body: { bodyText: longReplyBody },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'listed-longform-reply-create-1',
        },
        method: 'POST',
      },
    )

    expect(longTopicBody.length).toBeGreaterThan(4000)
    expect(longReplyBody.length).toBeGreaterThan(4000)
    expect(topicResponse.status).toBe(201)
    expect(replyResponse.status).toBe(201)
    await expect(replyResponse.json()).resolves.toMatchObject({
      idempotent: false,
      post: { bodyText: longReplyBody, postNumber: 2 },
      topic: { postCount: 2, title: 'Longform packet thread' },
    })
  })

  test('returns the original topic on idempotent create retry', async () => {
    const store = new ForumRouteStore()
    const initialPostCount = store.posts.length
    const initialTopicCount = store.topics.length
    const request = {
      body: {
        bodyText: 'Retry-safe body.',
        title: 'Retry safe topic',
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'void-topic-create-retry',
      },
      method: 'POST',
    }
    const first = await route(store, '/api/forum/forums/void/topics', request)
    const second = await route(store, '/api/forum/forums/void/topics', request)
    const conflict = await route(store, '/api/forum/forums/void/topics', {
      ...request,
      body: {
        bodyText: 'Retry-safe body with different content.',
        title: 'Retry safe topic',
      },
    })

    expect(first.status).toBe(201)
    expect(second.status).toBe(200)
    expect(conflict.status).toBe(409)
    await expect(second.json()).resolves.toMatchObject({
      firstPost: { bodyText: 'Retry-safe body.' },
      idempotent: true,
      topic: { title: 'Retry safe topic' },
    })
    await expect(conflict.json()).resolves.toMatchObject({
      error: 'idempotency_key_conflict',
    })
    expect(store.topics).toHaveLength(initialTopicCount + 1)
    expect(store.posts).toHaveLength(initialPostCount + 1)
  })

  test('creates a reply, bumps counters, and supports idempotent reply retry', async () => {
    const store = new ForumRouteStore()
    const topicResponse = await route(store, '/api/forum/forums/void/topics', {
      body: {
        bodyText: 'Reply root body.',
        title: 'Reply root',
      },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'void-reply-topic',
      },
      method: 'POST',
    })
    const topicBody = (await topicResponse.json()) as Readonly<{
      topic: Readonly<{ topicId: string }>
    }>
    const topicId = String(topicBody.topic.topicId)
    const replyRequest = {
      body: { bodyText: 'This is the reply body.' },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'void-reply-1',
      },
      method: 'POST',
    }
    const reply = await route(
      store,
      `/api/forum/topics/${topicId}/posts`,
      replyRequest,
    )
    const retry = await route(
      store,
      `/api/forum/topics/${topicId}/posts`,
      replyRequest,
    )
    const conflict = await route(store, `/api/forum/topics/${topicId}/posts`, {
      ...replyRequest,
      body: { bodyText: 'This reply body conflicts with the retry key.' },
    })

    expect(reply.status).toBe(201)
    await expect(reply.json()).resolves.toMatchObject({
      idempotent: false,
      post: { bodyText: 'This is the reply body.', postNumber: 2 },
      topic: { postCount: 2 },
    })
    expect(retry.status).toBe(200)
    expect(conflict.status).toBe(409)
    await expect(retry.json()).resolves.toMatchObject({
      idempotent: true,
      post: { bodyText: 'This is the reply body.', postNumber: 2 },
    })
    await expect(conflict.json()).resolves.toMatchObject({
      error: 'idempotency_key_conflict',
    })
    expect(store.forums[1]?.post_count).toBe(2)
  })

  test('denies unauthenticated, malformed, locked, and archived writes', async () => {
    const store = new ForumRouteStore()
    const noAuth = await route(store, '/api/forum/forums/void/topics', {
      body: { bodyText: 'No auth.', title: 'No auth' },
      headers: { 'idempotency-key': 'void-no-auth' },
      method: 'POST',
    })
    const malformed = await route(store, '/api/forum/forums/void/topics', {
      body: { bodyText: '', title: 'No' },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'void-malformed',
      },
      method: 'POST',
    })
    store.forums[1] = { ...store.forums[1]!, locked: 1 }
    const lockedForum = await route(store, '/api/forum/forums/void/topics', {
      body: { bodyText: 'Locked body.', title: 'Locked forum' },
      headers: {
        authorization: 'Bearer oa_agent_route_test',
        'idempotency-key': 'void-locked-forum',
      },
      method: 'POST',
    })
    store.forums[1] = { ...store.forums[1]!, locked: 0 }
    store.topics[0] = { ...store.topics[0]!, state: 'locked' }
    const lockedTopic = await route(
      store,
      '/api/forum/topics/55555555-5555-4555-8555-555555555555/posts',
      {
        body: { bodyText: 'Locked topic body.' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'void-locked-topic',
        },
        method: 'POST',
      },
    )
    store.topics[0] = { ...store.topics[0]!, state: 'archived' }
    const archivedTopic = await route(
      store,
      '/api/forum/topics/55555555-5555-4555-8555-555555555555/posts',
      {
        body: { bodyText: 'Archived topic body.' },
        headers: {
          authorization: 'Bearer oa_agent_route_test',
          'idempotency-key': 'void-archived-topic',
        },
        method: 'POST',
      },
    )

    expect(noAuth.status).toBe(401)
    expect(malformed.status).toBe(400)
    expect(lockedForum.status).toBe(423)
    expect(lockedTopic.status).toBe(423)
    expect(archivedTopic.status).toBe(404)
  })

  test('returns not found for unknown, archived, and hidden reads', async () => {
    const store = new ForumRouteStore()
    const unknown = await route(store, '/api/forum/forums/missing')
    store.forums[1] = { ...store.forums[1]!, discoverability: 'hidden' }
    const hidden = await route(store, '/api/forum/forums/void')
    store.topics[0] = {
      ...store.topics[0]!,
      archived_at: '2026-06-05T21:00:00.000Z',
    }
    const archived = await route(
      store,
      '/api/forum/topics/55555555-5555-4555-8555-555555555555',
    )

    expect(unknown.status).toBe(404)
    expect(hidden.status).toBe(404)
    expect(archived.status).toBe(404)
  })

  test('returns forbidden for non-public forum scopes and rejects non-GET methods', async () => {
    const store = new ForumRouteStore()
    store.forums[0] = { ...store.forums[0]!, visibility: 'team' }
    const scoped = await route(store, '/api/forum/forums/site-builder-help')
    const method = await route(store, '/api/forum', { method: 'POST' })

    expect(scoped.status).toBe(403)
    expect(method.status).toBe(405)
  })

  test('serves a per-thread Open Graph SVG image carrying the topic title', async () => {
    const store = new ForumRouteStore()
    const response = await route(
      store,
      '/og/forum/55555555-5555-4555-8555-555555555555.svg',
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toBe(
      'image/svg+xml; charset=utf-8',
    )
    const body = await response.text()
    expect(body).toContain('<svg')
    expect(body).toContain('width="1200"')
    expect(body).toContain('First Topic')
  })

  test('serves the branded default OG image for unknown/default topic ids', async () => {
    const store = new ForumRouteStore()
    const unknown = await route(
      store,
      '/og/forum/00000000-0000-4000-8000-000000000000.svg',
    )
    const fallback = await route(store, '/og/forum/default.svg')

    for (const response of [unknown, fallback]) {
      expect(response.status).toBe(200)
      expect(response.headers.get('content-type')).toBe(
        'image/svg+xml; charset=utf-8',
      )
      await expect(response.text()).resolves.toContain('OpenAgents Forum')
    }
  })

  test('rejects non-GET methods on the OG image route', async () => {
    const store = new ForumRouteStore()
    const response = await route(store, '/og/forum/default.svg', {
      method: 'POST',
    })

    expect(response.status).toBe(405)
  })
})
