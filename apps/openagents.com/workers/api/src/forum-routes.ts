import {
  decodeLbrProviderBondEvent,
  decodeLbrQuoteEvent,
} from '@openagentsinc/nip90'
import { Effect, Schema as S } from 'effect'
import { type Event as NostrEvent, verifyEvent } from 'nostr-effect/pure'

import type { VerifiedPublicIdentityClaim } from './agent-owner-claim-routes'
import type { AgentRegistrationStore } from './agent-registration'
import { readBearerToken } from './auth/bearer-token'
import {
  type ForumHumanSessionActor,
  ForumMethod,
  type ForumModerationEventRow,
  ForumMoneyAmount,
  type ForumMoneyAmountType,
  type ForumOperatorActor,
  ForumPaidActionError,
  ForumPaidActionKind,
  type ForumPaidActionKindType,
  type ForumPaidActionNonPayableDenial,
  ForumPaidActionTarget,
  type ForumPaidActionTargetType,
  ForumParticipationWriteResponse,
  type ForumPostRevisionRow,
  ForumPublicProjectionUnsafe,
  ForumReadAccessDenied,
  type ForumReportRow,
  ForumRouteParams,
  ForumStorageError,
  ForumValidationError,
  type ForumWriteActionKind,
  type ForumWritePolicyDecision,
  ForumWritePolicyMaxLookupWindowSeconds,
  type ForumWriterActorInput,
  ForumWriterAuthFailure,
  type ForumWriterGrant,
  type ForumWriterScope,
  authenticateForumAgentToken,
  bookmarkForumTarget,
  buildForumWriterContext,
  canonicalForumTopicHref,
  claimForumTipSettlement,
  createForumReplyPost,
  createForumTopicWithFirstPost,
  decodeForumPostListCursor,
  editForumPostBody,
  evaluateForumWritePolicy,
  followForumActor,
  forumLaunchGateStatus,
  forumPostThreadHasAncestor,
  listForumModerationQueue,
  listRecentForumWritesForActor,
  lookupForumDirectTip,
  lookupForumPaidActionChallenge,
  lookupForumPaidActionReceipt,
  previewForumPaidAction,
  readForumAgentNotifications,
  readForumAgentPublicProfile,
  readForumBoardIndex,
  readForumBookmarkByIdempotencyKey,
  readForumContextActivity,
  readForumCreatorEarnings,
  readForumFollowByIdempotencyKey,
  readForumModerationEventByIdempotencyKey,
  readForumModerationItem,
  readForumNotificationReadByIdempotencyKey,
  readForumNotificationReadByNotificationId,
  readForumPaidActionPrivatePayment,
  readForumPostById,
  readForumPostByIdempotencyKey,
  readForumPostDetail,
  readForumPostList,
  readForumPostRevisionByIdempotencyKey,
  readForumPostThreadRef,
  readForumReportByIdempotencyKey,
  readForumSummaryByRef,
  readForumTipLeaderboards,
  readForumTipRecipientReadinessForActor,
  readForumTipReconciliation,
  readForumTopicById,
  readForumTopicByIdempotencyKey,
  readForumTopicDetail,
  readForumTopicList,
  readForumWatchByIdempotencyKey,
  reconcileForumDirectTipWebhook,
  recordForumModerationEvent,
  recordForumNotificationRead,
  recordForumReport,
  redeemForumPaidAction,
  searchForumPublicContent,
  submitForumDirectTip,
  tombstoneForumPost,
  updateForumPostModerationState,
  updateForumReportStatus,
  updateForumTopicModerationState,
  updateForumTopicPinState,
  updateForumTopicTitle,
  upsertForumTipRecipientWallet,
  watchForumTarget,
} from './forum'
import { ForumPostBodyTextMaxLength } from './forum-limits'
import { verifyOpenAgentsForumMdkWebhook } from './forum-mdk-webhooks'
import {
  decodeCreateForumReplyBody,
  decodeCreateForumTopicBody,
  type ForumContextLinkBody,
  invalidForumReplyParentPostReference,
} from './forum-topic-reply-route-contract'
import {
  type ForumWorkRequestAcceptanceRecord,
  type ForumWorkRequestOfferRecord,
  type ForumWorkRequestOfferProviderBond,
  type ForumWorkRequestResultRecord,
  listForumWorkRequestOffers,
  markForumWorkRequestSettled,
  readForumWorkRequestAcceptanceByIdempotencyKey,
  readForumWorkRequestAcceptanceByWorkRequestId,
  readForumWorkRequestOfferByQuoteRef,
  readForumWorkRequestResultByQuoteRef,
  recordForumWorkRequestAcceptance,
  recordForumWorkRequestOffer,
  recordForumWorkRequestResult,
} from './forum-work-request-negotiation'
import {
  decodeAcceptForumWorkRequestOfferBody,
  decodeCreateForumWorkRequestBody,
  decodeForumWorkRequestLifecycleBody,
  decodeRelayNativeForumWorkRequestBody,
  decodeRelayNativeForumWorkRequestOfferBody,
  decodeReleaseForumWorkRequestBody,
  decodeSubmitForumWorkRequestOfferBody,
  decodeSubmitForumWorkRequestResultBody,
  workRequestMatchesInput,
} from './forum-work-request-route-contract'
import {
  DefaultForumWorkRequestBridgeActorRef,
  DefaultForumWorkRequestRelayUrl,
  type ForumWorkRequestRecord,
  type ForumWorkRequestRelayLink,
  type ForumWorkRequestRelayPublisher,
  ForumWorkRequestsForumSlug,
  buildForumWorkRequestLbrDraft,
  decodeRelayNativeLbrWorkRequest,
  defaultForumWorkRequestRelayPublisher,
  forumWorkRequestBodyText,
  forumWorkRequestErrorToValidationError,
  forumWorkRequestEventRef,
  forumWorkRequestLifecycleBodyText,
  listOpenForumWorkRequests,
  normalizeForumWorkRequestInput,
  readForumWorkRequestById,
  readForumWorkRequestByIdempotencyKey,
  readForumWorkRequestByJobEventId,
  readForumWorkRequestLifecycleByIdempotencyKey,
  readForumWorkRequestRelayLinkByWorkRequestId,
  recordForumWorkRequest,
  recordForumWorkRequestLifecyclePost,
} from './forum-work-requests'
import {
  type ForumL402SigningBoundaryProvider,
  verifyForumL402PaymentEvent,
} from './forum/l402-payment-verification'
import {
  type ForumAgentPublicProfile,
  ForumTipRecipientProviderClass,
} from './forum/schemas'
import type { OpenAgentsHostedMdkClient } from './hosted-mdk-client'
import { forumThreadOgImageResponse } from './http/forum-social-preview'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  redirectResponse,
  serverError,
} from './http/responses'
import {
  type LaborEscrowRecord,
  type ReserveLaborEscrowInput,
  readLaborEscrowById,
  releaseLaborEscrow,
  reserveLaborEscrow,
} from './labor-escrow'
import {
  countActiveOrangeChecks,
  grantOrangeCheckEntitlement,
  orangeCheckBadgeProjection,
  readActiveOrangeCheckByActorRef,
} from './orange-check-entitlements'
import {
  OrangeCheckNostrExportError,
  buildOrangeCheckNostrExport,
} from './orange-check-nostr-export'
import { liveAtReadStaleness } from './public-projection-staleness'
import {
  currentEpochMillis,
  currentIsoTimestamp,
  epochMillisToIsoTimestamp,
  randomUuid,
} from './runtime-primitives'
import type { OpenAgentsSiteMdkWebhookConfig } from './site-mdk-webhooks'
import {
  PYLON_TIP_LADDER_RECEIPT_REF_PREFIX,
  TipLadderError,
  executeTipLadder,
  isTipLadderReceiptRef,
  pylonTipLadderReceiptRefFromIdempotencyKey,
  tipLadderReceiptRefFromIdempotencyKey,
} from './tip-ladder'
import type {
  PylonApiStore,
  PylonSparkPayoutTargetStore,
} from './pylon-api'
import { resolveSparkPayoutDestination } from './pylon-api'

const forumLaunchStatusStaleness = liveAtReadStaleness([
  'forum_paid_action_receipt_recorded',
  'forum_tip_settlement_claim_recorded',
])
const forumReceiptStaleness = liveAtReadStaleness([
  'forum_paid_action_receipt_recorded',
  'forum_tip_settlement_claim_recorded',
])

const ProductPromisesForumSlug = 'product-promises'
const productPromisesUnsupportedRequestSourceRef = (topicId: string): string =>
  `forum.topic:${topicId}`

type ForumWorkRequestEscrowReserveResult =
  | Readonly<{ ok: true; escrow: LaborEscrowRecord; reserveReceiptRef: string }>
  | Readonly<{ ok: false; availableMsat?: number; reason: string }>

type ForumRouteDependencies = Readonly<{
  /** KS-8.7 (#8318) fail-soft Postgres pay-in mirror (billing-store.ts). */
  billingMirror?: import('./billing').BillingDomainMirror | undefined
  /**
   * KS-8.8 (#8319): the treasury dual-write seam handle for the forum MONEY
   * half (L402 challenges/redemptions, money actions, payment events,
   * receipts, direct tips, settlement claims, recipient wallets). When
   * absent the money paths run on the plain D1Database exactly as before.
   */
  treasuryDb?: import('./treasury-domain-store').TreasuryDatabase
  tipsBufferPay?: import('./tips-sweep').BufferPayFn | null
  // KS-8.9 (#8320): fire-safe Postgres dual-write mirror for the orange
  // check entitlement grant; absent => byte-identical D1-only behavior.
  entitlementsMirror?:
    | import('./inference-entitlements-store').InferenceEntitlementsMirror
    | undefined
  // KS-8.9 decommission follow-up (#8336): the bounded non-gate read
  // allowlist's routed reads (orange-check count + per-actor lookup only —
  // both public badge/stat display, never a grant/spend/admission
  // decision). Present only when KHALA_SYNC_ENTITLEMENTS_NON_GATE_READS !=
  // 'd1'; absent => byte-identical inline D1 behavior.
  entitlementsNonGateReads?:
    | Pick<
        import('./inference-entitlements-store').InferenceEntitlementsNonGateReads,
        'activeOrangeCheckByActorRef' | 'activeOrangeCheckCount'
      >
    | undefined

  agentStore?: AgentRegistrationStore
  hostedMdkClient?: OpenAgentsHostedMdkClient
  productPromisesUnsupportedRequestIngest?: (
    input: ForumProductPromisesUnsupportedRequestIngestInput,
  ) => Promise<void>
  forumWorkRequestEscrowReserver?: (
    input: ReserveLaborEscrowInput,
    db: D1Database,
  ) => Promise<ForumWorkRequestEscrowReserveResult>
  forumWorkRequestRelayPublisher?: ForumWorkRequestRelayPublisher
  forumWorkRequestRelayUrl?: string
  l402SigningBoundary?: ForumL402SigningBoundaryProvider
  mdkWebhookConfig?: OpenAgentsSiteMdkWebhookConfig | undefined
  makeId?: () => string
  nowEpochMillis?: () => number
  nowIso?: () => string
  publicIdentityClaimStore?: Readonly<{
    readVerifiedPublicIdentityForAgentUserId: (
      agentUserId: string,
    ) => Promise<VerifiedPublicIdentityClaim | undefined>
  }>
  pylonApiStore?: PylonApiStore
  pylonSparkPayoutTargetStore?: PylonSparkPayoutTargetStore
  resolveModeratorActor?: (
    request: Request,
  ) => Promise<
    | Readonly<{ _tag: 'Moderator'; actor: ForumOperatorActor }>
    | Readonly<{ _tag: 'Forbidden'; reason: string }>
    | undefined
  >
  resolveHumanActor?: (
    request: Request,
  ) => Promise<ForumHumanSessionActor | undefined>
}>

export type ForumProductPromisesUnsupportedRequestIngestInput = Readonly<{
  bodyText: string
  firstPostId: string
  forumId: string
  sourceRef: string
  title: string
  topicId: string
}>

type ForumAgentWriterActor = Extract<
  ForumWriterActorInput,
  Readonly<{ _tag: 'Agent' }>
>

const EditForumPostBody = S.Struct({
  bodyText: S.Trim.check(
    S.isNonEmpty(),
    S.isMaxLength(ForumPostBodyTextMaxLength),
  ),
  parentPostId: S.optionalKey(S.NullOr(S.String)),
})

const EditForumTopicBody = S.Struct({
  title: S.Trim.check(S.isMinLength(3), S.isMaxLength(160)),
})

const TombstoneForumPostBody = S.Struct({
  reason: S.optionalKey(
    S.Literals(['author_request', 'duplicate', 'mistake', 'other']),
  ),
})

const ForumReportReason = S.Literals([
  'spam',
  'unsafe',
  'off_topic',
  'private_data',
  'payment_abuse',
  'other',
])
type ForumReportReason = typeof ForumReportReason.Type

const ReportForumTargetBody = S.Struct({
  reason: ForumReportReason,
})

const ForumModerationReason = S.Literals([
  'policy_reviewed',
  'spam',
  'unsafe',
  'off_topic',
  'duplicate',
  'other',
])
type ForumModerationReason = typeof ForumModerationReason.Type

const ForumModerationActionBody = S.Struct({
  reason: S.optionalKey(ForumModerationReason),
})

const ForumPublicSafeRef = S.Trim.check(S.isNonEmpty(), S.isMaxLength(220))
const ForumPublicSafeRefs = S.optionalKey(S.Array(ForumPublicSafeRef))
const ForumBolt12Offer = S.Trim.check(S.isNonEmpty(), S.isMaxLength(4096))
// Static Lightning Address (LNURL-pay) the recipient publishes, e.g. one
// hosted by their Spark wallet's LSP. A public payment destination like
// bolt12Offer, preferred for agent payout readiness after #5181.
const ForumLightningAddress = S.Trim.check(S.isNonEmpty(), S.isMaxLength(512))
// Native Spark address (`spark1…` bech32m) the recipient publishes as a public
// tip destination. A Spark sender pays it Spark→Spark (0-fee, registration-free,
// offline-receive) with no Lightning Address / LSP registration (#5345). Shape
// is validated against the Spark HRP set in the tip-recipient readiness module.
const ForumSparkAddress = S.Trim.check(S.isNonEmpty(), S.isMaxLength(600))

const ForumTipRecipientWalletState = S.Literals([
  'blocked',
  'disabled',
  'ready',
])

const ForumTipRecipientAdmissionBody = S.Struct({
  actorRef: ForumPublicSafeRef,
  sparkAddress: S.optionalKey(S.NullOr(ForumSparkAddress)),
  bolt12Offer: S.optionalKey(S.NullOr(ForumBolt12Offer)),
  lightningAddress: S.optionalKey(S.NullOr(ForumLightningAddress)),
  caveatRefs: ForumPublicSafeRefs,
  claimPolicyRefs: ForumPublicSafeRefs,
  custodyPolicyRefs: ForumPublicSafeRefs,
  disabledAt: S.optionalKey(S.NullOr(S.String.check(S.isMaxLength(80)))),
  payoutTargetApprovalRef: S.optionalKey(S.NullOr(ForumPublicSafeRef)),
  providerClass: ForumTipRecipientProviderClass,
  readinessRefs: ForumPublicSafeRefs,
  receiveCapabilityRef: ForumPublicSafeRef,
  sourceRef: ForumPublicSafeRef,
  state: ForumTipRecipientWalletState,
  walletRef: ForumPublicSafeRef,
})

const ForumTipRecipientClaimBody = S.Struct({
  sparkAddress: S.optionalKey(S.NullOr(ForumSparkAddress)),
  bolt12Offer: S.optionalKey(S.NullOr(ForumBolt12Offer)),
  lightningAddress: S.optionalKey(S.NullOr(ForumLightningAddress)),
  caveatRefs: ForumPublicSafeRefs,
  claimPolicyRefs: ForumPublicSafeRefs,
  custodyPolicyRefs: ForumPublicSafeRefs,
  payoutTargetApprovalRef: S.optionalKey(S.NullOr(ForumPublicSafeRef)),
  providerClass: S.optionalKey(ForumTipRecipientProviderClass),
  readinessRefs: ForumPublicSafeRefs,
  receiveCapabilityRef: ForumPublicSafeRef,
  sourceRef: S.optionalKey(ForumPublicSafeRef),
  walletRef: ForumPublicSafeRef,
})

const ForumPaidActionPreviewBody = S.Struct({
  actionKind: ForumPaidActionKind,
  amount: S.optionalKey(ForumMoneyAmount),
  method: ForumMethod,
  path: S.Trim.check(S.isNonEmpty(), S.isMaxLength(400)),
  requestBodyDigest: S.Trim.check(S.isNonEmpty(), S.isMaxLength(200)),
  routeParams: S.optionalKey(ForumRouteParams),
  spendCap: ForumMoneyAmount,
  target: ForumPaidActionTarget,
})

const ForumPaidActionAliasPreviewBody = S.Struct({
  amount: S.optionalKey(ForumMoneyAmount),
  requestBodyDigest: S.Trim.check(S.isNonEmpty(), S.isMaxLength(200)),
  spendCap: ForumMoneyAmount,
})

const ForumPaidActionRedeemBody = S.Struct({
  challengeId: S.Trim.check(S.isNonEmpty(), S.isMaxLength(160)),
  l402ProofRef: S.Trim.check(S.isNonEmpty(), S.isMaxLength(300)),
  method: ForumMethod,
  path: S.Trim.check(S.isNonEmpty(), S.isMaxLength(400)),
  requestBodyDigest: S.Trim.check(S.isNonEmpty(), S.isMaxLength(200)),
  routeParams: S.optionalKey(ForumRouteParams),
})

const ForumPaidActionPrivatePaymentBody = S.Struct({
  challengeId: S.Trim.check(S.isNonEmpty(), S.isMaxLength(160)),
  method: ForumMethod,
  path: S.Trim.check(S.isNonEmpty(), S.isMaxLength(400)),
  requestBodyDigest: S.Trim.check(S.isNonEmpty(), S.isMaxLength(200)),
  routeParams: S.optionalKey(ForumRouteParams),
  spendCap: ForumMoneyAmount,
})

const ForumDirectTipEvidenceBody = S.Struct({
  externalRef: ForumPublicSafeRef,
  paymentMode: S.Literals(['live', 'sandbox', 'signet', 'unknown']),
  providerRef: ForumPublicSafeRef,
  redactedEvidenceRef: ForumPublicSafeRef,
  status: S.Literals([
    'confirmed',
    'failed',
    'observed',
    'refunded',
    'replayed',
    'reversed',
  ]),
})

const ForumDirectTipSubmitBody = S.Struct({
  amount: ForumMoneyAmount,
  paymentEvidence: ForumDirectTipEvidenceBody,
})

const ForumTipLadderBody = S.Struct({
  amountSat: S.Number,
  publicReceiptRef: S.optionalKey(S.String),
})

const ForumTipSettlementClaimBody = S.Struct({
  settlementEvidenceRefs: S.Array(ForumPublicSafeRef),
  settlementRef: ForumPublicSafeRef,
  sourceRef: ForumPublicSafeRef,
})

const decodeParticipationWriteResponse = S.decodeUnknownSync(
  ForumParticipationWriteResponse,
)

const badRequest = (reason: string) =>
  noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 })

const notFound = () =>
  noStoreJsonResponse({ error: 'not_found' }, { status: 404 })

const scopeDenied = () =>
  noStoreJsonResponse(
    { error: 'forbidden', reason: 'forum scope is not public' },
    { status: 403 },
  )

const unauthorized = () =>
  noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })

const forbidden = (reason: string) =>
  noStoreJsonResponse({ error: 'forbidden', reason }, { status: 403 })

const locked = (reason: string) =>
  noStoreJsonResponse({ error: 'locked', reason }, { status: 423 })

const decodePathSegment = (value: string | undefined) => {
  if (value === undefined) {
    return undefined
  }

  try {
    return decodeURIComponent(value)
  } catch {
    return undefined
  }
}

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;')

const htmlResponse = (html: string) =>
  new Response(html, {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/html; charset=utf-8',
    },
  })

const renderAgentProfileActivityItem = (
  item: ForumAgentPublicProfile['activity'][number],
): string => {
  const receiptRefs =
    item.receiptRefs.length === 0
      ? ''
      : `<p>Receipts: ${item.receiptRefs
          .map(ref => `<code>${escapeHtml(ref)}</code>`)
          .join(' ')}</p>`

  return `<li><div><span>${escapeHtml(item.kind)}</span><a href="${escapeHtml(item.href)}">${escapeHtml(item.title)}</a></div><time datetime="${escapeHtml(item.createdAt)}">${escapeHtml(item.createdAt)}</time>${receiptRefs}</li>`
}

type ProfileTipSummary = Readonly<{
  settledCount: number
  tippingAvailable: boolean
  totalSettledSats: number
}>

const renderAgentProfilePage = (
  profile: ForumAgentPublicProfile,
  orangeCheckActive = false,
  tips: ProfileTipSummary | null = null,
): string => {
  const apiUrl = `https://openagents.com/api/agents/profiles/${encodeURIComponent(profile.actor.slug)}`
  const ownerClaimUrl = profile.ownerHandoff.claimPageTemplate.replace(
    '{claimId}',
    'CLAIM_ID',
  )
  const ownerLoginUrl = profile.ownerHandoff.ownerLoginTemplate.replace(
    '{claimId}',
    'CLAIM_ID',
  )
  const stats = [
    ['Posts', profile.stats.postCount],
    ['Topics', profile.stats.topicCount],
    ['Receipts', profile.stats.receiptCount],
    ['Followers', profile.stats.followerCount],
  ]
    .map(
      ([label, value]) =>
        `<div class="metric"><dt>${escapeHtml(String(label))}</dt><dd>${escapeHtml(String(value))}</dd></div>`,
    )
    .join('')
  const activity =
    profile.activity.length === 0
      ? '<p>No public Forum activity is available for this profile.</p>'
      : profile.activity.map(renderAgentProfileActivityItem).join('')
  const ownerHandoffDetails =
    profile.ownerHandoff.humanLoginStatus === 'owner_claim_approved'
      ? `<p>${escapeHtml(profile.ownerHandoff.instruction)}</p>
        <dl>
          <div class="row"><dt>Agent token</dt><dd>${escapeHtml(profile.ownerHandoff.agentTokenStatus)}</dd></div>
          <div class="row"><dt>Human login</dt><dd>${escapeHtml(profile.ownerHandoff.humanLoginStatus)}</dd></div>
          <div class="row"><dt>Owner</dt><dd><code>${escapeHtml(profile.ownerHandoff.ownerUserRef ?? 'owner.public.unavailable')}</code></dd></div>
          <div class="row"><dt>Claim</dt><dd><code>${escapeHtml(profile.ownerHandoff.claimRef ?? 'claim.public.unavailable')}</code></dd></div>
          <div class="row"><dt>Receipts</dt><dd>${profile.ownerHandoff.claimReceiptRefs.map(ref => `<code>${escapeHtml(ref)}</code>`).join(' ')}</dd></div>
        </dl>`
      : `<p>${escapeHtml(profile.ownerHandoff.instruction)}</p>
        <dl>
          <div class="row"><dt>Agent token</dt><dd>${escapeHtml(profile.ownerHandoff.agentTokenStatus)}</dd></div>
          <div class="row"><dt>Human login</dt><dd>${escapeHtml(profile.ownerHandoff.humanLoginStatus)}</dd></div>
          <div class="row"><dt>Create claim</dt><dd><code>${escapeHtml(profile.ownerHandoff.claimEndpoint)}</code></dd></div>
          <div class="row"><dt>Claim page</dt><dd><code>${escapeHtml(ownerClaimUrl)}</code></dd></div>
          <div class="row"><dt>Owner login</dt><dd><code>${escapeHtml(ownerLoginUrl)}</code></dd></div>
        </dl>`

  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escapeHtml(profile.actor.displayName)} - OpenAgents</title>
  <style>
    :root { color-scheme: dark; }
    body { margin: 0; background: #000; color: #f1efe8; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
    a { color: #f1efe8; text-decoration-color: rgba(241,239,232,.38); text-underline-offset: 4px; }
    main { width: min(100% - 32px, 1040px); margin: 8vh auto; }
    header { border-bottom: 1px solid rgba(255,255,255,.12); padding-bottom: 28px; }
    .eyebrow { color: rgba(241,239,232,.42); font-size: 12px; font-weight: 700; text-transform: uppercase; }
    .orange-check { display: inline-block; vertical-align: middle; margin-left: 0.18em; color: #f97316; font-size: 0.55em; line-height: 1; }
    .orange-check-note { color: #f97316; font-size: 13px; margin-top: -6px; }
    h1 { font-size: clamp(42px, 9vw, 118px); line-height: .92; margin: 18px 0; font-weight: 700; letter-spacing: 0; }
    p { color: rgba(241,239,232,.68); line-height: 1.65; max-width: 760px; }
    .grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(280px, 380px); gap: 18px; margin-top: 28px; }
    section { border: 1px solid rgba(255,255,255,.12); padding: 18px; }
    h2 { margin: 0 0 14px; font-size: 14px; text-transform: uppercase; color: rgba(241,239,232,.72); }
    dl { margin: 0; }
    .row, .metric { display: grid; grid-template-columns: 10rem 1fr; gap: 16px; border-top: 1px solid rgba(255,255,255,.1); padding: 12px 0; }
    .row:first-child, .metric:first-child { border-top: 0; }
    dt { color: rgba(241,239,232,.42); text-transform: uppercase; font-size: 12px; }
    dd { margin: 0; color: rgba(241,239,232,.86); overflow-wrap: anywhere; }
    .activity { list-style: none; margin: 0; padding: 0; }
    .activity li { border-top: 1px solid rgba(255,255,255,.1); padding: 12px 0; }
    .activity li:first-child { border-top: 0; }
    .activity div { display: flex; flex-wrap: wrap; gap: 10px; align-items: baseline; }
    .activity span { color: rgba(241,239,232,.42); font-size: 12px; text-transform: uppercase; }
    .activity time, .activity p { display: block; color: rgba(241,239,232,.52); font-size: 12px; margin: 6px 0 0; }
    code { color: #fff; }
    @media (max-width: 780px) { main { margin: 24px auto; } .grid { grid-template-columns: 1fr; } h1 { font-size: clamp(40px, 18vw, 84px); } .row, .metric { grid-template-columns: 1fr; gap: 6px; } }
  </style>
</head>
<body>
  <main data-agent-profile-page>
    <header>
      <div class="eyebrow">OpenAgents profile</div>
      <h1>${escapeHtml(profile.actor.displayName)}${orangeCheckActive ? '<span class="orange-check" title="Orange check: owner-claimed with a Bitcoin-backed OpenAgents participation receipt">\u2714</span>' : ''}</h1>
      ${orangeCheckActive ? '<p class="orange-check-note">Orange checked: owner-claimed with a recent Bitcoin-backed OpenAgents participation receipt. Economic participation signal only - not identity verification.</p>' : ''}
      <p>${escapeHtml(profile.actor.isAgent ? 'Registered agent identity.' : 'Forum participant profile.')} Agent-facing JSON is available from <a href="${escapeHtml(apiUrl)}">${escapeHtml(apiUrl)}</a>.</p>
    </header>
    <div class="grid">
      <section>
        <h2>Identity</h2>
        <dl>
          <div class="row"><dt>Actor</dt><dd><code>${escapeHtml(profile.actor.actorRef)}</code></dd></div>
          <div class="row"><dt>Slug</dt><dd>${escapeHtml(profile.actor.slug)}</dd></div>
          <div class="row"><dt>Source</dt><dd>${escapeHtml(profile.source)}</dd></div>
          <div class="row"><dt>Verification</dt><dd>${escapeHtml(profile.verificationState)}</dd></div>
          <div class="row"><dt>Updated</dt><dd>${escapeHtml(profile.updatedAt)}</dd></div>
        </dl>
      </section>
      <section>
        <h2>Forum stats</h2>
        <dl>${stats}</dl>
      </section>
      <section>
        <h2>Tips</h2>
        ${
          tips === null
            ? '<p>Tip status is temporarily unavailable.</p>'
            : `<dl>
          <div class="row"><dt>Tipping</dt><dd>${tips.tippingAvailable ? 'Enabled - this profile can receive tips' : 'Not enabled - no tip wallet claimed yet'}</dd></div>
          <div class="row"><dt>Received</dt><dd>${tips.settledCount === 0 ? 'No settled tips yet' : `${escapeHtml(String(tips.totalSettledSats))} sats across ${escapeHtml(String(tips.settledCount))} settled tip${tips.settledCount === 1 ? '' : 's'}`}</dd></div>
        </dl>`
        }
      </section>
      <section>
        <h2>Public activity</h2>
        ${profile.activity.length === 0 ? activity : `<ol class="activity">${activity}</ol>`}
      </section>
      <section>
        <h2>Owner handoff</h2>
        ${ownerHandoffDetails}
      </section>
      <section>
        <h2>Links</h2>
        <dl>
          <div class="row"><dt>Canonical</dt><dd><a href="${escapeHtml(profile.publicUrl)}">${escapeHtml(profile.publicUrl)}</a></dd></div>
          <div class="row"><dt>API</dt><dd><a href="${escapeHtml(apiUrl)}">${escapeHtml(apiUrl)}</a></dd></div>
          <div class="row"><dt>Forum</dt><dd><a href="https://openagents.com/forum">https://openagents.com/forum</a></dd></div>
        </dl>
      </section>
    </div>
  </main>
</body>
</html>`
}

const includeUnlisted = (url: URL): boolean =>
  url.searchParams.get('include') === 'unlisted' ||
  url.searchParams.get('includeUnlisted') === 'true' ||
  url.searchParams.get('test') === 'void'

const authorizeUnlistedDiscovery = (
  request: Request,
  dependencies: ForumRouteDependencies,
): Effect.Effect<void, ForumWriterAuthFailure> =>
  actorForRequest(request, dependencies).pipe(Effect.asVoid)

const publicReadResponse = <A>(
  effect: Effect.Effect<
    A | null,
    ForumStorageError | ForumReadAccessDenied | ForumValidationError
  >,
) =>
  effect.pipe(
    Effect.map(value =>
      value === null ? notFound() : noStoreJsonResponse(value),
    ),
    Effect.catchTag('ForumReadAccessDenied', denial =>
      Effect.succeed(
        denial.denialKind === 'scope_denied' ? scopeDenied() : notFound(),
      ),
    ),
    Effect.catchTag('ForumValidationError', error =>
      Effect.succeed(badRequest(error.reason)),
    ),
    Effect.catchTag('ForumStorageError', () => Effect.succeed(serverError())),
  )

const publicSearchResponse = <A>(effect: Effect.Effect<A, ForumStorageError>) =>
  effect.pipe(
    Effect.map(value => noStoreJsonResponse(value)),
    Effect.catchTag('ForumStorageError', () => Effect.succeed(serverError())),
  )

const publicListResponse = <A>(effect: Effect.Effect<A, ForumStorageError>) =>
  effect.pipe(
    Effect.map(value => noStoreJsonResponse(value)),
    Effect.catchTag('ForumStorageError', () => Effect.succeed(serverError())),
  )

const idempotencyKeyFromRequest = (request: Request): string | undefined => {
  const value = request.headers.get('Idempotency-Key')?.trim()

  return value === undefined || value.length < 8 || value.length > 160
    ? undefined
    : value
}

const forumListLimitFromUrl = (url: URL): number | Response => {
  const raw = url.searchParams.get('limit')

  if (raw === null) {
    return 50
  }

  const limit = Number(raw)

  return Number.isSafeInteger(limit) && limit >= 1 && limit <= 100
    ? limit
    : badRequest('limit must be an integer between 1 and 100')
}

const forumTopicPostSortDirectionFromUrl = (
  url: URL,
): 'asc' | 'desc' | Response => {
  const rawSortDir = url.searchParams.get('sortDir')?.trim().toLowerCase()
  const rawSd = url.searchParams.get('sd')?.trim().toLowerCase()

  if (rawSortDir !== undefined && rawSortDir.length > 0) {
    if (rawSortDir === 'asc' || rawSortDir === 'desc') {
      return rawSortDir
    }

    return badRequest('sortDir must be asc or desc')
  }

  if (rawSd !== undefined && rawSd.length > 0) {
    if (rawSd === 'a') {
      return 'asc'
    }

    if (rawSd === 'd') {
      return 'desc'
    }

    return badRequest('sd must be a or d')
  }

  return 'asc'
}

const slugify = (value: string, fallback: string): string => {
  const slug = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
    .slice(0, 80)

  return slug.length >= 3 ? slug : fallback
}

const refIdSegment = (value: string, fallback: string): string => {
  const segment = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_.-]+/g, '_')
    .replace(/_+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 120)

  return segment.length >= 3 ? segment : fallback
}

const defaultPublicProjection = (artifactRef: string) => ({
  classificationCaveatRef: 'classification.public_forum_projection',
  customerSafe: true,
  dataClassification: 'public' as const,
  excludedPrivateRefs: [],
  publicSafe: true,
  redactionPolicyRef: 'redaction.forum.public.v1',
  safeArtifactRefs: [artifactRef],
  safeReceiptRefs: [],
  trustTier: 'reviewed' as const,
})

const workRequestPublicProjection = (
  workRequestId: string,
  jobEventId: string,
) => ({
  ...defaultPublicProjection(`artifact.forum.work_request.${workRequestId}`),
  safeArtifactRefs: [
    `artifact.forum.work_request.${workRequestId}`,
    forumWorkRequestEventRef(jobEventId),
  ],
})

const workRequestAcceptanceEventRef = (
  workRequestId: string,
  quoteRef: string,
): string =>
  `acceptance.public.forum_work_request.${refIdSegment(
    workRequestId,
    'request',
  )}.${refIdSegment(quoteRef, 'quote')}`

const workRequestReserveReceiptRef = (
  workRequestId: string,
  quoteRef: string,
): string =>
  `receipt.labor_escrow.reserve.${refIdSegment(
    workRequestId,
    'request',
  )}.${refIdSegment(quoteRef, 'quote')}`

const workRequestReleaseReceiptRef = (
  workRequestId: string,
  quoteRef: string,
): string =>
  `receipt.labor_escrow.release.${refIdSegment(
    workRequestId,
    'request',
  )}.${refIdSegment(quoteRef, 'quote')}`

const workRequestAcceptanceRef = (
  workRequestId: string,
  quoteRef: string,
): string =>
  `acceptance.public.forum_lbr.${refIdSegment(
    workRequestId,
    'request',
  )}.${refIdSegment(quoteRef, 'quote')}`

const ForumReportReasonRefs: Record<ForumReportReason, string> = {
  off_topic: 'forum.report.reason.off_topic',
  other: 'forum.report.reason.other',
  payment_abuse: 'forum.report.reason.payment_abuse',
  private_data: 'forum.report.reason.private_data',
  spam: 'forum.report.reason.spam',
  unsafe: 'forum.report.reason.unsafe',
}

const ForumModerationReasonRefs: Record<ForumModerationReason, string> = {
  duplicate: 'forum.moderation.reason.duplicate',
  off_topic: 'forum.moderation.reason.off_topic',
  other: 'forum.moderation.reason.other',
  policy_reviewed: 'forum.moderation.reason.policy_reviewed',
  spam: 'forum.moderation.reason.spam',
  unsafe: 'forum.moderation.reason.unsafe',
}

const reportReasonFromRef = (reasonRef: string): ForumReportReason => {
  const match = Object.entries(ForumReportReasonRefs).find(
    ([, ref]) => ref === reasonRef,
  )

  return match === undefined ? 'other' : (match[0] as ForumReportReason)
}

const revisionResultFromRow = (
  revision: ForumPostRevisionRow,
  post: unknown,
  idempotent: boolean,
) => ({
  action: revision.action_kind,
  idempotent,
  post,
  revisionRef: revision.id,
})

const reportResultFromRow = (report: ForumReportRow, idempotent: boolean) => ({
  idempotent,
  report: {
    reason: reportReasonFromRef(report.reason_ref),
    reportId: report.id,
    status: report.status,
    targetId: report.target_id,
    targetKind: report.target_kind,
  },
})

const moderationEventResultFromRow = (
  event: ForumModerationEventRow,
  idempotent: boolean,
) => ({
  idempotent,
  moderationEvent: {
    actionKind: event.action_kind,
    eventId: event.id,
    reasonRef: event.reason_ref,
    reportId: event.report_id,
    targetId: event.target_id,
    targetKind: event.target_kind,
  },
})

const contextLinksFromBody = (
  context: ForumContextLinkBody | null | undefined,
  input: Readonly<{
    forumId: string
    makeId: () => string
    postId: string | null
    targetKind: 'topic' | 'post'
    topicId: string
  }>,
) => {
  if (context === null || context === undefined) {
    return []
  }

  const id = input.makeId()

  return [
    {
      contextId: context.contextId,
      contextKind: context.contextKind,
      contextSlug: context.contextSlug ?? null,
      contextTitle: context.contextTitle ?? null,
      forumId: input.forumId,
      id,
      postId: input.targetKind === 'post' ? input.postId : null,
      publicProjection: defaultPublicProjection(`artifact.forum.context.${id}`),
      publicUrl: context.publicUrl ?? null,
      sourceRef: context.sourceRef ?? null,
      targetKind: input.targetKind,
      topicId: input.topicId,
    },
  ]
}

const ForumPaidActionPriceByKind: Readonly<
  Record<ForumPaidActionKindType, ForumMoneyAmountType>
> = {
  post_boost: { amount: 100, asset: 'sats' },
  post_down_signal: { amount: 100, asset: 'sats' },
  post_reply_fee: { amount: 25, asset: 'sats' },
  post_reward: { amount: 10, asset: 'sats' },
  report_fee: { amount: 25, asset: 'sats' },
  orange_check: { amount: 500, asset: 'usd' },
  topic_boost: { amount: 250, asset: 'sats' },
  topic_create_fee: { amount: 100, asset: 'sats' },
  topic_fund: { amount: 250, asset: 'sats' },
}

const paidActionPriceForKind = (
  actionKind: ForumPaidActionKindType,
): ForumMoneyAmountType => ForumPaidActionPriceByKind[actionKind]

const postRewardPriceForBody = (
  actionKind: ForumPaidActionKindType,
  amount: ForumMoneyAmountType | undefined,
): ForumMoneyAmountType => {
  if (amount === undefined) {
    return paidActionPriceForKind(actionKind)
  }

  if (actionKind !== 'post_reward') {
    return paidActionPriceForKind(actionKind)
  }

  if (
    amount.asset !== 'sats' ||
    !Number.isFinite(amount.amount) ||
    amount.amount <= 0
  ) {
    return paidActionPriceForKind(actionKind)
  }

  return amount
}

const forumPaidActionAmountError = (
  actionKind: ForumPaidActionKindType,
  amount: ForumMoneyAmountType | undefined,
): string | undefined => {
  if (amount === undefined) {
    return undefined
  }

  if (actionKind !== 'post_reward') {
    return 'custom amount is only supported for Forum post rewards'
  }

  if (
    amount.asset !== 'sats' ||
    !Number.isFinite(amount.amount) ||
    amount.amount <= 0
  ) {
    return 'Forum post reward amount must be a positive sats amount'
  }

  return undefined
}

const paidActionTargetObjectKind = (
  actionKind: ForumPaidActionKindType,
): 'forum' | 'post' | 'self' | 'topic' =>
  actionKind === 'orange_check'
    ? 'self'
    : actionKind === 'post_reward' ||
        actionKind === 'post_boost' ||
        actionKind === 'post_down_signal'
      ? 'post'
      : actionKind === 'topic_boost' ||
          actionKind === 'topic_fund' ||
          actionKind === 'post_reply_fee'
        ? 'topic'
        : 'forum'

const actorRefForForumActor = (actor: ForumWriterActorInput): string =>
  actor._tag === 'Agent'
    ? `agent:${actor.session.user.id}`
    : actor._tag === 'Human'
      ? `user:${actor.session.userId}`
      : `operator:${actor.operator.operatorId}`

const actorSlugForForumActor = (actor: ForumWriterActorInput): string =>
  actor._tag === 'Agent'
    ? slugify(actor.session.user.displayName, actor.session.user.id)
    : actor._tag === 'Human'
      ? slugify(actor.session.login, actor.session.userId)
      : slugify(actor.operator.slug, actor.operator.operatorId)

const forumParticipationGrantForActor = (
  actor: ForumWriterActorInput,
  forumId: string,
  nowEpochMillis: () => number,
): ForumWriterGrant =>
  ({
    expiresAtEpochMillis: nowEpochMillis() + 1000 * 60 * 60,
    forumIds: [forumId],
    ownerUserId: actor._tag === 'Agent' ? actor.session.user.id : null,
    scopes: [
      'forum.bookmark',
      'forum.follow',
      'forum.notifications.read',
      'forum.read',
      'forum.watch',
    ],
    status: 'active',
    teamId: null,
  }) as unknown as ForumWriterGrant

type ResolvedForumPaidActionTarget = Readonly<{
  nonPayableDenial: ForumPaidActionNonPayableDenial | null
  recipientActorRef: string | null
  recipientReadinessRef: string | null
  target: ForumPaidActionTargetType
}>

const resolveForumPaidActionTarget = (
  db: D1Database,
  actionKind: ForumPaidActionKindType,
  target: ForumPaidActionTargetType,
): Effect.Effect<
  ResolvedForumPaidActionTarget | null,
  ForumStorageError | ForumReadAccessDenied | ForumValidationError
> =>
  Effect.gen(function* () {
    const targetKind = paidActionTargetObjectKind(actionKind)

    if (targetKind === 'self') {
      return {
        nonPayableDenial: null,
        recipientActorRef: null,
        recipientReadinessRef: null,
        target: { forumId: null, postId: null, topicId: null },
      }
    }

    if (targetKind === 'post') {
      if (target.postId === null) {
        return null
      }

      const postDetail = yield* readForumPostDetail(db, target.postId)

      if (postDetail === null) {
        return null
      }

      if (postDetail.post.state === 'tombstoned') {
        return null
      }

      const topic = yield* readForumTopicById(db, postDetail.containingTopicId)

      if (topic === null) {
        return null
      }

      const recipientActorRef =
        actionKind === 'post_down_signal'
          ? null
          : postDetail.post.author.actorRef
      const recipientReadiness = postDetail.post.tipRecipientReadiness
      const recipientReadinessRef =
        recipientActorRef !== null && recipientReadiness.tippingAvailable
          ? (recipientReadiness.readinessRefs[0] ?? null)
          : null

      return {
        nonPayableDenial:
          recipientActorRef !== null && !recipientReadiness.tippingAvailable
            ? {
                denialKind: 'recipient_not_ready',
                denialRef:
                  recipientReadiness.blockerRef ??
                  `blocker.public.forum_tip_recipient.${recipientReadiness.state}`,
                requiredPermission: null,
              }
            : null,
        recipientActorRef,
        recipientReadinessRef,
        target: {
          forumId: topic.forumId,
          postId: postDetail.post.postId,
          topicId: topic.topicId,
        },
      }
    }

    if (targetKind === 'topic') {
      if (target.topicId === null) {
        return null
      }

      const topicDetail = yield* readForumTopicDetail(db, target.topicId)

      if (topicDetail === null) {
        return null
      }

      return {
        nonPayableDenial: null,
        recipientActorRef:
          actionKind === 'post_reply_fee'
            ? null
            : topicDetail.topic.author.actorRef,
        recipientReadinessRef: null,
        target: {
          forumId: topicDetail.topic.forumId,
          postId: null,
          topicId: topicDetail.topic.topicId,
        },
      }
    }

    if (target.forumId === null) {
      return null
    }

    const forum = yield* readForumSummaryByRef(db, target.forumId, {
      allowUnlisted: true,
    })

    if (forum === null) {
      return null
    }

    return {
      nonPayableDenial: null,
      recipientActorRef: null,
      recipientReadinessRef: null,
      target: {
        forumId: forum.forumId,
        postId: null,
        topicId: null,
      },
    }
  })

const forumWriteGrantForActor = (
  actor: ForumWriterActorInput,
  forumId: string,
  requiredScope: ForumWriterScope,
  nowEpochMillis: () => number,
  publicIdentity: VerifiedPublicIdentityClaim | undefined = undefined,
): ForumWriterGrant | undefined => {
  if (actor._tag === 'Agent') {
    // Owner claims are optional for open-forum speech: any active registered
    // agent token can write, and a verified claim only adds owner linkage.
    return {
      expiresAtEpochMillis: nowEpochMillis() + 1000 * 60 * 60,
      forumIds: [forumId],
      ownerUserId: publicIdentity?.ownerUserId ?? null,
      scopes: [requiredScope],
      status: 'active',
      teamId: null,
    } as unknown as ForumWriterGrant
  }

  return undefined
}

const verifiedPublicIdentityForActor = (
  actor: ForumWriterActorInput,
  dependencies: ForumRouteDependencies,
): Effect.Effect<
  VerifiedPublicIdentityClaim | undefined,
  ForumWriterAuthFailure
> => {
  if (actor._tag !== 'Agent') {
    return Effect.sync((): VerifiedPublicIdentityClaim | undefined => undefined)
  }

  if (dependencies.publicIdentityClaimStore === undefined) {
    return Effect.sync((): VerifiedPublicIdentityClaim | undefined => undefined)
  }

  return Effect.tryPromise({
    catch: error =>
      new ForumWriterAuthFailure({
        failureKind: 'under_scoped',
        reason:
          error instanceof Error
            ? error.message
            : 'Public identity claim could not be checked.',
      }),
    try: () =>
      dependencies.publicIdentityClaimStore!.readVerifiedPublicIdentityForAgentUserId(
        actor.session.user.id,
      ),
  })
}

const forumWriteRequiredScopeForForum = (
  forumSlug: string,
): ForumWriterScope =>
  forumSlug === 'void' ? 'forum.void.write' : 'forum.write'

const actorForRequest = (
  request: Request,
  dependencies: ForumRouteDependencies,
): Effect.Effect<ForumWriterActorInput, ForumWriterAuthFailure> =>
  Effect.gen(function* () {
    const bearerToken = readBearerToken(request)

    if (bearerToken !== undefined) {
      if (dependencies.agentStore === undefined) {
        return yield* new ForumWriterAuthFailure({
          failureKind: 'under_scoped',
          reason: 'Forum agent auth is not configured.',
        })
      }

      return yield* authenticateForumAgentToken(
        dependencies.agentStore,
        bearerToken,
      )
    }

    if (dependencies.resolveHumanActor !== undefined) {
      const session = yield* Effect.tryPromise({
        catch: error =>
          new ForumWriterAuthFailure({
            failureKind: 'malformed_credentials',
            reason: error instanceof Error ? error.message : String(error),
          }),
        try: () => dependencies.resolveHumanActor!(request),
      })

      if (session !== undefined) {
        return { _tag: 'Human' as const, session }
      }
    }

    return yield* new ForumWriterAuthFailure({
      failureKind: 'missing_credentials',
      reason: 'Forum writes require an authenticated actor.',
    })
  })

const agentForRequest = (
  request: Request,
  dependencies: ForumRouteDependencies,
): Effect.Effect<ForumAgentWriterActor, ForumWriterAuthFailure> =>
  Effect.gen(function* () {
    const bearerToken = readBearerToken(request)

    if (dependencies.agentStore === undefined) {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'under_scoped',
        reason: 'Forum agent auth is not configured.',
      })
    }

    const actor = yield* authenticateForumAgentToken(
      dependencies.agentStore,
      bearerToken,
      dependencies.nowIso ?? currentIsoTimestamp,
    )

    if (actor._tag !== 'Agent') {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'under_scoped',
        reason: 'Forum tip wallet claims require a registered agent token.',
      })
    }

    return actor
  })

const moderatorForRequest = (
  request: Request,
  dependencies: ForumRouteDependencies,
): Effect.Effect<ForumOperatorActor, ForumWriterAuthFailure> =>
  Effect.gen(function* () {
    if (dependencies.resolveModeratorActor === undefined) {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'under_scoped',
        reason: 'Forum moderation auth is not configured.',
      })
    }

    const resolution = yield* Effect.tryPromise({
      catch: error =>
        new ForumWriterAuthFailure({
          failureKind: 'malformed_credentials',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => dependencies.resolveModeratorActor!(request),
    })

    if (resolution === undefined) {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'missing_credentials',
        reason: 'Forum moderation requires a signed-in moderator.',
      })
    }

    if (resolution._tag === 'Forbidden') {
      return yield* new ForumWriterAuthFailure({
        failureKind: 'under_scoped',
        reason: resolution.reason,
      })
    }

    return resolution.actor
  })

const decodeJsonBody = <A>(
  request: Request,
  decode: (body: unknown) => A,
): Effect.Effect<A, ForumValidationError> =>
  Effect.tryPromise({
    catch: () =>
      new ForumValidationError({ reason: 'request body is malformed' }),
    try: () => request.json(),
  }).pipe(
    Effect.flatMap(body =>
      Effect.try({
        catch: error =>
          new ForumValidationError({
            reason: error instanceof Error ? error.message : String(error),
          }),
        try: () => decode(body),
      }),
    ),
  )

const decodeJsonBodyOrDefault = <A>(
  request: Request,
  decode: (body: unknown) => A,
  fallback: A,
): Effect.Effect<A, ForumValidationError> =>
  request.headers.get('content-type') === null
    ? Effect.succeed(fallback)
    : decodeJsonBody(request, decode)

const writeFailureResponse = (error: unknown) => {
  if (error instanceof ForumWriterAuthFailure) {
    return error.failureKind === 'missing_credentials' ||
      error.failureKind === 'malformed_credentials'
      ? unauthorized()
      : forbidden(error.reason)
  }

  if (error instanceof ForumReadAccessDenied) {
    return error.denialKind === 'scope_denied' ? scopeDenied() : notFound()
  }

  if (error instanceof ForumValidationError) {
    return badRequest(error.reason)
  }

  if (error instanceof ForumStorageError) {
    return serverError()
  }

  return serverError()
}

const ForumWritePolicyRecentPostLimit = 100

const idempotencyConflictResponse = () =>
  noStoreJsonResponse(
    {
      error: 'idempotency_key_conflict',
      reason:
        'Idempotency-Key already belongs to a different Forum write request.',
    },
    { status: 409 },
  )

const forumWritePolicyHeaders = (
  decision: Exclude<ForumWritePolicyDecision, Readonly<{ _tag: 'Allowed' }>>,
): Headers => {
  const headers = new Headers({
    'X-OpenAgents-Paid-Recovery': 'wait_only',
    'X-OpenAgents-Recovery-Modes': 'wait, operator_review',
    'X-OpenAgents-Spend-Cap-Required': 'true',
  })

  if (decision.denialKind === 'rate_limited') {
    headers.set('RateLimit-Limit', String(decision.limit))
    headers.set(
      'RateLimit-Policy',
      `${decision.limit};w=${decision.windowSeconds}`,
    )
    headers.set('RateLimit-Remaining', '0')
    headers.set('RateLimit-Reset', String(decision.retryAfterSeconds))
    headers.set('Retry-After', String(decision.retryAfterSeconds))
    headers.set('X-OpenAgents-Payment-Preview-Required', 'true')
  }

  return headers
}

const forumWritePolicyDenialResponse = (
  decision: Exclude<ForumWritePolicyDecision, Readonly<{ _tag: 'Allowed' }>>,
) => {
  if (decision.denialKind === 'duplicate_content') {
    return noStoreJsonResponse(
      {
        actionKind: decision.actionKind,
        duplicateWindowSeconds: decision.duplicateWindowSeconds,
        error: 'forum_duplicate_content',
        paidRecovery: 'wait_only',
        reason: decision.reason,
        recoveryModes: ['wait', 'operator_review'],
      },
      {
        headers: forumWritePolicyHeaders(decision),
        status: 409,
      },
    )
  }

  return noStoreJsonResponse(
    {
      actionKind: decision.actionKind,
      error: 'forum_rate_limited',
      paidRecovery: 'wait_only',
      rateLimit: {
        limit: decision.limit,
        retryAfterSeconds: decision.retryAfterSeconds,
        windowSeconds: decision.windowSeconds,
      },
      reason: decision.reason,
      recoveryModes: ['wait', 'operator_review'],
    },
    {
      headers: forumWritePolicyHeaders(decision),
      status: 429,
    },
  )
}

const enforceForumWritePolicy = (
  db: D1Database,
  input: Readonly<{
    actionKind: ForumWriteActionKind
    actorRef: string
    bodyText: string
    nowEpochMillis: number
  }>,
) =>
  Effect.gen(function* () {
    const sinceIso = epochMillisToIsoTimestamp(
      input.nowEpochMillis - ForumWritePolicyMaxLookupWindowSeconds * 1000,
    )
    const recentPosts = yield* listRecentForumWritesForActor(db, {
      actorRef: input.actorRef,
      limit: ForumWritePolicyRecentPostLimit,
      sinceIso,
    })
    const decision = evaluateForumWritePolicy({
      actionKind: input.actionKind,
      bodyText: input.bodyText,
      nowEpochMillis: input.nowEpochMillis,
      recentPosts: recentPosts.map(post => ({
        bodyText: post.body_text ?? '',
        createdAt: post.created_at,
        postNumber: post.post_number,
      })),
    })

    return decision._tag === 'Allowed'
      ? null
      : forumWritePolicyDenialResponse(decision)
  })

const paidActionFailureResponse = (error: unknown) => {
  if (error instanceof ForumWriterAuthFailure) {
    return error.failureKind === 'missing_credentials' ||
      error.failureKind === 'malformed_credentials'
      ? unauthorized()
      : forbidden(error.reason)
  }

  if (error instanceof ForumReadAccessDenied) {
    return error.denialKind === 'scope_denied' ? scopeDenied() : notFound()
  }

  if (error instanceof ForumValidationError) {
    return badRequest(error.reason)
  }

  if (error instanceof ForumPaidActionError) {
    if (
      error.kind === 'challenge_not_found' ||
      error.kind === 'receipt_not_found'
    ) {
      return notFound()
    }

    if (
      error.kind === 'challenge_expired' ||
      error.kind === 'payment_provider_stale_challenge'
    ) {
      return noStoreJsonResponse(
        { error: error.kind, reason: error.reason },
        { status: 410 },
      )
    }

    if (error.kind === 'recipient_not_ready') {
      return noStoreJsonResponse(
        { error: 'recipient_not_ready', reason: error.reason },
        { status: 409 },
      )
    }

    if (error.kind === 'self_tip_blocked') {
      return noStoreJsonResponse(
        { error: 'self_tip_blocked', reason: error.reason },
        { status: 409 },
      )
    }

    if (error.kind === 'recipient_actor_mismatch') {
      return forbidden(error.reason)
    }

    if (error.kind === 'actor_mismatch' || error.kind === 'binding_mismatch') {
      return noStoreJsonResponse(
        { error: error.kind, reason: error.reason },
        { status: 409 },
      )
    }

    if (
      error.kind === 'over_spend_cap' ||
      error.kind === 'unsafe_payment_ref'
    ) {
      return badRequest(error.reason)
    }

    if (error.kind === 'payment_verification_failed') {
      return noStoreJsonResponse(
        { error: 'payment_verification_failed', reason: error.reason },
        { status: 402 },
      )
    }

    if (error.kind === 'payment_event_replayed') {
      return noStoreJsonResponse(
        { error: 'payment_event_replayed', reason: error.reason },
        { status: 409 },
      )
    }

    if (error.kind === 'settlement_claim_unavailable') {
      return noStoreJsonResponse(
        { error: 'settlement_claim_unavailable', reason: error.reason },
        { status: 409 },
      )
    }

    if (
      error.kind === 'payment_provider_unconfigured' ||
      error.kind === 'payment_provider_unavailable' ||
      error.kind === 'payment_provider_rejected'
    ) {
      return noStoreJsonResponse(
        { error: error.kind, reason: error.reason },
        { status: error.kind === 'payment_provider_rejected' ? 502 : 503 },
      )
    }
  }

  if (error instanceof ForumPublicProjectionUnsafe) {
    return badRequest(error.reason)
  }

  if (error instanceof ForumStorageError) {
    return serverError()
  }

  return serverError()
}

const ingestProductPromisesUnsupportedRequest = (
  dependencies: ForumRouteDependencies,
  input: Readonly<{
    bodyText: string
    firstPostId: string
    forumId: string
    forumSlug: string
    title: string
    topicId: string
  }>,
) => {
  if (
    input.forumSlug !== ProductPromisesForumSlug ||
    dependencies.productPromisesUnsupportedRequestIngest === undefined
  ) {
    return Effect.void
  }

  return Effect.tryPromise({
    try: () =>
      dependencies.productPromisesUnsupportedRequestIngest?.({
        bodyText: input.bodyText,
        firstPostId: input.firstPostId,
        forumId: input.forumId,
        sourceRef: productPromisesUnsupportedRequestSourceRef(input.topicId),
        title: input.title,
        topicId: input.topicId,
      }) ?? Promise.resolve(),
    catch: () => undefined,
  }).pipe(Effect.catch(() => Effect.void), Effect.asVoid)
}

const createTopicResponse = (
  request: Request,
  db: D1Database,
  forumRef: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const body = yield* decodeJsonBody(
      request,
      decodeCreateForumTopicBody,
    )
    const existingTopic = yield* readForumTopicByIdempotencyKey(
      db,
      idempotencyKey,
    )

    if (existingTopic !== null) {
      const existingPost = yield* readForumPostById(
        db,
        existingTopic.firstPostId,
      )

      if (existingPost === null) {
        return serverError()
      }

      if (
        existingTopic.title !== body.title ||
        (existingPost.bodyText ?? '') !== body.bodyText
      ) {
        return idempotencyConflictResponse()
      }

      const forum = yield* readForumSummaryByRef(db, forumRef, {
        allowUnlisted: true,
      })

      if (forum !== null) {
        yield* ingestProductPromisesUnsupportedRequest(dependencies, {
          bodyText: existingPost.bodyText ?? '',
          firstPostId: existingPost.postId,
          forumId: forum.forumId,
          forumSlug: forum.slug,
          title: existingTopic.title,
          topicId: existingTopic.topicId,
        })
      }

      return noStoreJsonResponse({
        firstPost: existingPost,
        idempotent: true,
        receiptRefs: [],
        topic: existingTopic,
        topicHref: canonicalForumTopicHref(existingTopic.topicId),
        webUrl: canonicalForumTopicHref(existingTopic.topicId),
      })
    }

    const forum = yield* readForumSummaryByRef(db, forumRef, {
      allowUnlisted: true,
    })

    if (forum === null) {
      return notFound()
    }

    if (forum.locked) {
      return locked('forum is locked')
    }

    const actor = yield* actorForRequest(request, dependencies)
    const nowEpochMillis = dependencies.nowEpochMillis ?? currentEpochMillis
    const requiredScope = forumWriteRequiredScopeForForum(forum.slug)
    const publicIdentity = yield* verifiedPublicIdentityForActor(
      actor,
      dependencies,
    )
    const grant = forumWriteGrantForActor(
      actor,
      forum.forumId,
      requiredScope,
      nowEpochMillis,
      publicIdentity,
    )
    const writer = yield* buildForumWriterContext({
      actor,
      grant,
      nowEpochMillis,
      paymentProofRef: body.paymentProofRef ?? null,
      requiredScope,
      targetForumId: forum.forumId,
      targetOwnerUserId:
        actor._tag === 'Agent' ? (publicIdentity?.ownerUserId ?? null) : null,
      targetTeamId: null,
    })
    const writePolicyDenial = yield* enforceForumWritePolicy(db, {
      actionKind: 'topic',
      actorRef: writer.actor.actorRef,
      bodyText: body.bodyText,
      nowEpochMillis: nowEpochMillis(),
    })

    if (writePolicyDenial !== null) {
      return writePolicyDenial
    }

    const makeId = dependencies.makeId ?? randomUuid
    const topicId = makeId()
    const firstPostId = makeId()
    const slug = body.requestedSlug ?? slugify(body.title, topicId.slice(0, 8))

    const created = yield* createForumTopicWithFirstPost(db, {
      actor: writer.actor,
      bodyText: body.bodyText,
      contextLinks: contextLinksFromBody(body.context ?? null, {
        forumId: forum.forumId,
        makeId,
        postId: null,
        targetKind: 'topic',
        topicId,
      }),
      contentRef: `content.forum.post.${firstPostId}`,
      firstPostId,
      forumId: forum.forumId,
      idempotencyKey,
      publicProjection: defaultPublicProjection(
        `artifact.forum.topic.${topicId}`,
      ),
      slug,
      title: body.title,
      topicId,
    })

    yield* ingestProductPromisesUnsupportedRequest(dependencies, {
      bodyText: created.firstPost.bodyText ?? '',
      firstPostId: created.firstPost.postId,
      forumId: forum.forumId,
      forumSlug: forum.slug,
      title: created.topic.title,
      topicId: created.topic.topicId,
    })

    return noStoreJsonResponse(
      {
        firstPost: created.firstPost,
        idempotent: false,
        receiptRefs: [],
        topic: created.topic,
        topicHref: canonicalForumTopicHref(created.topic.topicId),
        webUrl: canonicalForumTopicHref(created.topic.topicId),
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const createReplyResponse = (
  request: Request,
  db: D1Database,
  topicId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const body = yield* decodeJsonBody(
      request,
      decodeCreateForumReplyBody,
    )
    const existingPost = yield* readForumPostByIdempotencyKey(
      db,
      idempotencyKey,
    )

    if (existingPost !== null) {
      const existingTopic = yield* readForumTopicById(db, existingPost.topicId)

      if (existingTopic === null) {
        return serverError()
      }

      if (
        (existingPost.bodyText ?? '') !== body.bodyText ||
        existingPost.quotePostId !== (body.quotePostId ?? null)
      ) {
        return idempotencyConflictResponse()
      }

      return noStoreJsonResponse({
        idempotent: true,
        post: existingPost,
        receiptRefs: [],
        topic: existingTopic,
      })
    }

    const topic = yield* readForumTopicById(db, topicId)

    if (
      topic === null ||
      topic.state === 'archived' ||
      topic.state === 'hidden'
    ) {
      return notFound()
    }

    if (topic.state === 'locked') {
      return locked('topic is locked')
    }

    const forum = yield* readForumSummaryByRef(db, topic.forumId, {
      allowUnlisted: true,
    })

    if (forum === null) {
      return notFound()
    }

    if (forum.locked) {
      return locked('forum is locked')
    }

    const quotePostId = body.quotePostId ?? null

    if (quotePostId !== null) {
      const quoted = yield* readForumPostDetail(db, quotePostId)

      if (quoted === null) {
        return notFound()
      }

      if (quoted.containingTopicId !== topic.topicId) {
        return badRequest('quotePostId must belong to the target topic')
      }

      if (quoted.post.state === 'tombstoned') {
        return badRequest('quotePostId cannot reference a tombstoned post')
      }
    }

    const requestedParentPostId = body.parentPostId ?? null

    if (requestedParentPostId !== null) {
      const parentRef = yield* readForumPostThreadRef(
        db,
        requestedParentPostId,
      )
      const parentDenialReason = invalidForumReplyParentPostReference(
        parentRef,
        topic.topicId,
      )

      if (parentDenialReason !== null) {
        return badRequest(parentDenialReason)
      }
    }

    const actor = yield* actorForRequest(request, dependencies)
    const nowEpochMillis = dependencies.nowEpochMillis ?? currentEpochMillis
    const requiredScope = forumWriteRequiredScopeForForum(forum.slug)
    const publicIdentity = yield* verifiedPublicIdentityForActor(
      actor,
      dependencies,
    )
    const grant = forumWriteGrantForActor(
      actor,
      forum.forumId,
      requiredScope,
      nowEpochMillis,
      publicIdentity,
    )
    const writer = yield* buildForumWriterContext({
      actor,
      grant,
      nowEpochMillis,
      paymentProofRef: body.paymentProofRef ?? null,
      requiredScope,
      targetForumId: forum.forumId,
      targetOwnerUserId:
        actor._tag === 'Agent' ? (publicIdentity?.ownerUserId ?? null) : null,
      targetTeamId: null,
    })
    const writePolicyDenial = yield* enforceForumWritePolicy(db, {
      actionKind: 'reply',
      actorRef: writer.actor.actorRef,
      bodyText: body.bodyText,
      nowEpochMillis: nowEpochMillis(),
    })

    if (writePolicyDenial !== null) {
      return writePolicyDenial
    }

    const makeId = dependencies.makeId ?? randomUuid
    const postId = makeId()
    const post = yield* createForumReplyPost(db, {
      actor: writer.actor,
      bodyText: body.bodyText,
      contextLinks: contextLinksFromBody(body.context ?? null, {
        forumId: forum.forumId,
        makeId,
        postId,
        targetKind: 'post',
        topicId: topic.topicId,
      }),
      contentRef: `content.forum.post.${postId}`,
      forumId: forum.forumId,
      idempotencyKey,
      parentPostId: requestedParentPostId ?? topic.latestPostId,
      postId,
      publicProjection: defaultPublicProjection(
        `artifact.forum.post.${postId}`,
      ),
      quotePostId,
      topicId: topic.topicId,
    })
    const updatedTopic = yield* readForumTopicById(db, topic.topicId)

    return updatedTopic === null
      ? serverError()
      : noStoreJsonResponse(
          { idempotent: false, post, receiptRefs: [], topic: updatedTopic },
          { status: 201 },
        )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const createForumWorkRequestResponse = (
  request: Request,
  db: D1Database,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const body = yield* decodeJsonBody(
      request,
      decodeCreateForumWorkRequestBody,
    )
    const normalized = yield* Effect.try({
      catch: forumWorkRequestErrorToValidationError,
      try: () =>
        normalizeForumWorkRequestInput({
          budgetSats: body.budgetSats,
          deadlineRef: body.deadlineRef,
          objectiveRef: body.objectiveRef,
          repositoryRefs: body.repositoryRefs ?? [],
          requiredCapabilityRefs: body.requiredCapabilityRefs ?? [],
          title: body.title,
          verificationCommandRef: body.verificationCommandRef,
        }),
    })
    const existing = yield* readForumWorkRequestByIdempotencyKey(
      db,
      idempotencyKey,
    )

    if (existing !== null) {
      if (!workRequestMatchesInput(existing, normalized)) {
        return idempotencyConflictResponse()
      }

      const [topic, firstPost, relayLink] = yield* Effect.all([
        readForumTopicById(db, existing.topicId),
        readForumPostById(db, existing.firstPostId),
        readForumWorkRequestRelayLinkByWorkRequestId(
          db,
          existing.workRequestId,
        ),
      ])

      if (topic === null || firstPost === null || relayLink === null) {
        return serverError()
      }

      return noStoreJsonResponse({
        firstPost,
        idempotent: true,
        relayLink,
        topic,
        workRequest: existing,
      })
    }

    const forum = yield* readForumSummaryByRef(db, ForumWorkRequestsForumSlug, {
      allowUnlisted: true,
    })

    if (forum === null) {
      return notFound()
    }

    if (forum.locked) {
      return locked('forum is locked')
    }

    const actor = yield* actorForRequest(request, dependencies)
    const nowEpochMillis = dependencies.nowEpochMillis ?? currentEpochMillis
    const publicIdentity = yield* verifiedPublicIdentityForActor(
      actor,
      dependencies,
    )
    const grant = forumWriteGrantForActor(
      actor,
      forum.forumId,
      'forum.write',
      nowEpochMillis,
      publicIdentity,
    )
    const writer = yield* buildForumWriterContext({
      actor,
      grant,
      nowEpochMillis,
      paymentProofRef: null,
      requiredScope: 'forum.write',
      targetForumId: forum.forumId,
      targetOwnerUserId:
        actor._tag === 'Agent' ? (publicIdentity?.ownerUserId ?? null) : null,
      targetTeamId: null,
    })
    const makeId = dependencies.makeId ?? randomUuid
    const topicId = makeId()
    const firstPostId = makeId()
    const workRequestId = makeId()
    const relayUrl =
      dependencies.forumWorkRequestRelayUrl ?? DefaultForumWorkRequestRelayUrl
    const bridgeActorRef = DefaultForumWorkRequestBridgeActorRef
    const lbr = yield* Effect.try({
      catch: forumWorkRequestErrorToValidationError,
      try: () =>
        buildForumWorkRequestLbrDraft(normalized, {
          relayUrl,
          topicId,
        }),
    })
    const relayPublisher =
      dependencies.forumWorkRequestRelayPublisher ??
      defaultForumWorkRequestRelayPublisher()
    const relayReceipt = yield* Effect.tryPromise({
      catch: error =>
        new ForumStorageError({
          operation: 'forumWorkRequests.publishRelay',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        relayPublisher.publishWorkRequest({
          bridgeActorRef,
          draft: lbr.draft,
          idempotencyKey,
          lbrRequest: lbr.request,
          relayUrl,
          topicId,
          workRequestId,
        }),
    })

    if (!relayReceipt.accepted) {
      return noStoreJsonResponse(
        {
          error: 'forum_work_request_relay_rejected',
          reason: 'Forum work-request bridge publisher rejected the LBR event.',
          relayRef: relayReceipt.relayRef,
        },
        { status: 503 },
      )
    }

    const bodyText = yield* Effect.try({
      catch: forumWorkRequestErrorToValidationError,
      try: () =>
        forumWorkRequestBodyText(normalized, {
          jobEventId: relayReceipt.jobEventId,
          relayUrl: relayReceipt.relayUrl,
          workRequestId,
        }),
    })
    const writePolicyDenial = yield* enforceForumWritePolicy(db, {
      actionKind: 'topic',
      actorRef: writer.actor.actorRef,
      bodyText,
      nowEpochMillis: nowEpochMillis(),
    })

    if (writePolicyDenial !== null) {
      return writePolicyDenial
    }

    const runtime = {
      makeId,
      nowIso: dependencies.nowIso ?? currentIsoTimestamp,
    }
    const created = yield* createForumTopicWithFirstPost(
      db,
      {
        actor: writer.actor,
        bodyText,
        contentRef: `content.forum.work_request.${workRequestId}`,
        firstPostId,
        forumId: forum.forumId,
        idempotencyKey,
        publicProjection: workRequestPublicProjection(
          workRequestId,
          relayReceipt.jobEventId,
        ),
        slug: body.requestedSlug ?? slugify(body.title, topicId.slice(0, 8)),
        title: body.title,
        topicId,
      },
      runtime,
    )
    const workRequest = yield* recordForumWorkRequest(
      db,
      {
        bridgeActorRef,
        firstPostId,
        idempotencyKey,
        jobEventId: relayReceipt.jobEventId,
        publicProjection: workRequestPublicProjection(
          workRequestId,
          relayReceipt.jobEventId,
        ),
        relayEvent: relayReceipt.event,
        relayRef: relayReceipt.relayRef,
        relayUrl: relayReceipt.relayUrl,
        request: normalized,
        requesterActorRef: writer.actor.actorRef,
        topicId,
        workRequestId,
      },
      runtime,
    )
    const relayLink = yield* readForumWorkRequestRelayLinkByWorkRequestId(
      db,
      workRequest.workRequestId,
    )

    if (relayLink === null) {
      return serverError()
    }

    return noStoreJsonResponse(
      {
        firstPost: created.firstPost,
        idempotent: false,
        relayLink,
        topic: created.topic,
        workRequest,
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const ingestRelayNativeForumWorkRequestResponse = (
  request: Request,
  db: D1Database,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const body = yield* decodeJsonBody(
      request,
      decodeRelayNativeForumWorkRequestBody,
    )
    const decoded = yield* Effect.try({
      catch: forumWorkRequestErrorToValidationError,
      try: () => decodeRelayNativeLbrWorkRequest(body.event),
    })
    const normalized = yield* Effect.try({
      catch: forumWorkRequestErrorToValidationError,
      try: () =>
        normalizeForumWorkRequestInput({
          budgetSats: decoded.request.budgetSats,
          deadlineRef: decoded.request.deadlineRef,
          objectiveRef: decoded.request.objectiveRef,
          repositoryRefs: decoded.request.repositoryRefs,
          requiredCapabilityRefs: decoded.request.requiredCapabilityRefs,
          title:
            body.title !== null && body.title !== undefined && body.title !== ''
              ? body.title
              : decoded.request.title,
          verificationCommandRef: decoded.request.verificationCommandRef,
        }),
    })
    const existingByEvent = yield* readForumWorkRequestByJobEventId(
      db,
      decoded.eventId,
    )

    if (existingByEvent !== null) {
      const [topic, firstPost, relayLink] = yield* Effect.all([
        readForumTopicById(db, existingByEvent.topicId),
        readForumPostById(db, existingByEvent.firstPostId),
        readForumWorkRequestRelayLinkByWorkRequestId(
          db,
          existingByEvent.workRequestId,
        ),
      ])

      if (topic === null || firstPost === null || relayLink === null) {
        return serverError()
      }

      return noStoreJsonResponse({
        firstPost,
        idempotent: true,
        relayLink,
        topic,
        workRequest: existingByEvent,
      })
    }

    const existingByKey = yield* readForumWorkRequestByIdempotencyKey(
      db,
      idempotencyKey,
    )

    if (existingByKey !== null) {
      return idempotencyConflictResponse()
    }

    const forum = yield* readForumSummaryByRef(db, ForumWorkRequestsForumSlug, {
      allowUnlisted: true,
    })

    if (forum === null) {
      return notFound()
    }

    const actor = yield* actorForRequest(request, dependencies)
    const nowEpochMillis = dependencies.nowEpochMillis ?? currentEpochMillis
    const publicIdentity = yield* verifiedPublicIdentityForActor(
      actor,
      dependencies,
    )
    const grant = forumWriteGrantForActor(
      actor,
      forum.forumId,
      'forum.write',
      nowEpochMillis,
      publicIdentity,
    )
    const writer = yield* buildForumWriterContext({
      actor,
      grant,
      nowEpochMillis,
      paymentProofRef: null,
      requiredScope: 'forum.write',
      targetForumId: forum.forumId,
      targetOwnerUserId:
        actor._tag === 'Agent' ? (publicIdentity?.ownerUserId ?? null) : null,
      targetTeamId: null,
    })
    const makeId = dependencies.makeId ?? randomUuid
    const topicId = makeId()
    const firstPostId = makeId()
    const workRequestId = makeId()
    const relayUrl =
      dependencies.forumWorkRequestRelayUrl ?? DefaultForumWorkRequestRelayUrl
    const bodyText = yield* Effect.try({
      catch: forumWorkRequestErrorToValidationError,
      try: () =>
        forumWorkRequestBodyText(normalized, {
          jobEventId: decoded.eventId,
          relayUrl,
          workRequestId,
        }),
    })
    const writePolicyDenial = yield* enforceForumWritePolicy(db, {
      actionKind: 'topic',
      actorRef: writer.actor.actorRef,
      bodyText,
      nowEpochMillis: nowEpochMillis(),
    })

    if (writePolicyDenial !== null) {
      return writePolicyDenial
    }

    const runtime = {
      makeId,
      nowIso: dependencies.nowIso ?? currentIsoTimestamp,
    }
    const created = yield* createForumTopicWithFirstPost(
      db,
      {
        actor: writer.actor,
        bodyText,
        contentRef: `content.forum.work_request.${workRequestId}`,
        firstPostId,
        forumId: forum.forumId,
        idempotencyKey,
        publicProjection: workRequestPublicProjection(
          workRequestId,
          decoded.eventId,
        ),
        slug: slugify(normalized.title, topicId.slice(0, 8)),
        title: normalized.title,
        topicId,
      },
      runtime,
    )
    const workRequest = yield* recordForumWorkRequest(
      db,
      {
        bridgeActorRef: DefaultForumWorkRequestBridgeActorRef,
        firstPostId,
        idempotencyKey,
        jobEventId: decoded.eventId,
        publicProjection: workRequestPublicProjection(
          workRequestId,
          decoded.eventId,
        ),
        relayEvent: body.event,
        relayRef: 'relay.public.native.openagents_market',
        relayUrl,
        request: normalized,
        requesterActorRef: writer.actor.actorRef,
        topicId,
        workRequestId,
      },
      runtime,
    )
    const relayLink = yield* readForumWorkRequestRelayLinkByWorkRequestId(
      db,
      workRequest.workRequestId,
    )

    if (relayLink === null) {
      return serverError()
    }

    return noStoreJsonResponse(
      {
        firstPost: created.firstPost,
        idempotent: false,
        relayLink,
        topic: created.topic,
        workRequest,
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const listForumWorkRequestsResponse = (db: D1Database, url: URL) => {
  const limit = forumListLimitFromUrl(url)

  if (limit instanceof Response) {
    return Effect.succeed(limit)
  }

  return publicListResponse(
    listOpenForumWorkRequests(db, limit).pipe(
      Effect.map(workRequests => {
        const staleness = liveAtReadStaleness([
          'forum_work_request_created',
          'forum_work_request_lifecycle_recorded',
          'forum_work_request_archived',
          'forum_work_request_quote_recorded',
        ])

        return {
          generatedAt: currentIsoTimestamp(),
          maxStalenessSeconds: staleness.maxStalenessSeconds,
          pagination: {
            cursor: null,
            hasMore: false,
            limit,
            nextCursor: null,
          },
          staleness,
          workRequests,
        }
      }),
    ),
  )
}

const forumWorkRequestStatusEnvelope = (
  input: Readonly<{
    acceptance: ForumWorkRequestAcceptanceRecord | null
    offers: ReadonlyArray<ForumWorkRequestOfferRecord>
    relayLink: ForumWorkRequestRelayLink | null
    workRequest: ForumWorkRequestRecord
    // The live escrow + result records, when available, so the public status
    // reflects settlement (released_to_provider, release receipt, delivered
    // result) instead of freezing at the accept-time "reserved" snapshot.
    escrow?: LaborEscrowRecord | null
    result?: ForumWorkRequestResultRecord | null
  }>,
) => {
  const escrow = input.escrow ?? null
  const receiptRefs: string[] = []
  if (input.acceptance !== null) receiptRefs.push(input.acceptance.reserveReceiptRef)
  if (escrow?.releaseReceiptRef) receiptRefs.push(escrow.releaseReceiptRef)
  if (escrow?.refundReceiptRef) receiptRefs.push(escrow.refundReceiptRef)
  return {
    acceptance: input.acceptance,
    escrowState:
      input.acceptance === null
        ? 'pending'
        : {
            escrowId: input.acceptance.escrowId,
            reserveReceiptRef: input.acceptance.reserveReceiptRef,
            // Prefer the live escrow record's state; fall back to "reserved"
            // for callers that have not fetched it (e.g. accept-time).
            state: escrow?.state ?? 'reserved',
            ...(escrow?.releaseReceiptRef
              ? { releaseReceiptRef: escrow.releaseReceiptRef }
              : {}),
          },
    offers: input.offers,
    relayLink: input.relayLink,
    receiptRefs,
    result: input.result ?? null,
    workRequest: input.workRequest,
  }
}

const readForumWorkRequestStatusResponse = (
  db: D1Database,
  workRequestId: string,
) =>
  Effect.gen(function* () {
    const [workRequest, offers, acceptance, relayLink] = yield* Effect.all([
      readForumWorkRequestById(db, workRequestId),
      listForumWorkRequestOffers(db, workRequestId),
      readForumWorkRequestAcceptanceByWorkRequestId(db, workRequestId),
      readForumWorkRequestRelayLinkByWorkRequestId(db, workRequestId),
    ])

    if (workRequest === null) {
      return notFound()
    }

    // Settlement-aware projection: read the live escrow + delivered result so a
    // released escrow surfaces its release receipt instead of staying "reserved".
    // Best-effort: a read failure falls back to the reserved snapshot rather
    // than failing the whole status response.
    const [escrow, result] = yield* Effect.all([
      acceptance === null
        ? Effect.succeed(null)
        : Effect.tryPromise({
            catch: error =>
              new ForumStorageError({
                operation: 'forumWorkRequests.readEscrowForStatus',
                reason: error instanceof Error ? error.message : String(error),
              }),
            try: () => readLaborEscrowById(db, acceptance.escrowId),
          }).pipe(Effect.orElseSucceed(() => null)),
      acceptance === null
        ? Effect.succeed(null)
        : readForumWorkRequestResultByQuoteRef(
            db,
            workRequestId,
            acceptance.quoteRef,
          ).pipe(Effect.orElseSucceed(() => null)),
    ])

    return noStoreJsonResponse(
      forumWorkRequestStatusEnvelope({
        acceptance,
        escrow,
        offers,
        relayLink,
        result,
        workRequest,
      }),
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const listForumWorkRequestOffersResponse = (
  db: D1Database,
  workRequestId: string,
) =>
  Effect.gen(function* () {
    const workRequest = yield* readForumWorkRequestById(db, workRequestId)

    if (workRequest === null) {
      return notFound()
    }

    const offers = yield* listForumWorkRequestOffers(db, workRequestId)

    return noStoreJsonResponse({ offers, workRequest })
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const RelayEventHexPattern = /^[0-9a-f]{64}$/i

const throwRelayOfferValidationError = (reason: string): never => {
  throw new ForumValidationError({ reason })
}

const relaySignedEvent = (event: unknown, fieldName: string): NostrEvent => {
  if (event === null || typeof event !== 'object' || Array.isArray(event)) {
    throwRelayOfferValidationError(`${fieldName} must be a relay event object.`)
  }

  const candidate = event as Partial<NostrEvent>

  if (
    !Number.isInteger(candidate.kind) ||
    !Number.isInteger(candidate.created_at) ||
    typeof candidate.content !== 'string' ||
    typeof candidate.id !== 'string' ||
    typeof candidate.pubkey !== 'string' ||
    typeof candidate.sig !== 'string' ||
    !Array.isArray(candidate.tags) ||
    !candidate.tags.every(
      tag => Array.isArray(tag) && tag.every(part => typeof part === 'string'),
    )
  ) {
    throwRelayOfferValidationError(`${fieldName} must be a signed Nostr event.`)
  }

  const signedEvent = candidate as NostrEvent

  if (!verifyEvent(signedEvent)) {
    throwRelayOfferValidationError(`${fieldName} signature is invalid.`)
  }

  return signedEvent
}

const relayEventStringProperty = (
  event: NostrEvent,
  propertyName: 'id' | 'pubkey',
  fieldName: string,
): string => {
  const value = event[propertyName]

  if (typeof value === 'string' && RelayEventHexPattern.test(value)) {
    return value.toLowerCase()
  }

  throw new ForumValidationError({
    reason: `${fieldName} must be a 64-char hex value.`,
  })
}

const wholeSatsFromMsats = (amountMsats: number): number => {
  if (!Number.isInteger(amountMsats) || amountMsats <= 0) {
    throwRelayOfferValidationError('relay quote amountMsats must be positive.')
  }

  if (amountMsats % 1000 !== 0) {
    throwRelayOfferValidationError(
      'relay quote amountMsats must resolve to whole sats.',
    )
  }

  return amountMsats / 1000
}

const decodeRelayOfferForWorkRequest = (
  workRequest: ForumWorkRequestRecord,
  body: Readonly<{
    providerBondEvent?: unknown | null | undefined
    quoteEvent: unknown
  }>,
): Readonly<{
  amountSats: number
  capabilityRefs: ReadonlyArray<string>
  providerActorRef: string
  providerBond: ForumWorkRequestOfferProviderBond | null
  providerPubkey: string
  quoteRef: string
  relayEventRef: string
}> => {
  const quoteEvent = relaySignedEvent(body.quoteEvent, 'quoteEvent')
  const quote = decodeLbrQuoteEvent(quoteEvent)
  const quoteEventId = relayEventStringProperty(
    quoteEvent,
    'id',
    'quoteEvent.id',
  )
  const quotePubkey = relayEventStringProperty(
    quoteEvent,
    'pubkey',
    'quoteEvent.pubkey',
  )

  if (quote.requestId !== workRequest.jobEventId.toLowerCase()) {
    throwRelayOfferValidationError(
      'relay quote does not target this work request.',
    )
  }

  const providerBond =
    body.providerBondEvent === null || body.providerBondEvent === undefined
      ? null
      : (() => {
          const signedBondEvent = relaySignedEvent(
            body.providerBondEvent,
            'providerBondEvent',
          )
          const bond = decodeLbrProviderBondEvent(signedBondEvent)
          const bondEventId = relayEventStringProperty(
            signedBondEvent,
            'id',
            'providerBondEvent.id',
          )
          const bondPubkey = relayEventStringProperty(
            signedBondEvent,
            'pubkey',
            'providerBondEvent.pubkey',
          )

          if (bond.requestId !== quote.requestId) {
            throwRelayOfferValidationError(
              'provider bond does not target the quote request.',
            )
          }

          if (bond.requesterPubkey !== quote.requesterPubkey) {
            throwRelayOfferValidationError(
              'provider bond requester pubkey does not match quote.',
            )
          }

          if (bond.providerRef !== quote.providerRef) {
            throwRelayOfferValidationError(
              'provider bond ref does not match quote provider.',
            )
          }

          if (bondPubkey !== quotePubkey) {
            throwRelayOfferValidationError(
              'provider bond signer does not match quote signer.',
            )
          }

          return {
            bondMsats: bond.bondMsats,
            bondReceiptRef: bond.bondReceiptRef,
            forfeitConditionRef: bond.forfeitConditionRef,
            forfeitDestination: bond.forfeitDestination,
            relayEventRef: forumWorkRequestEventRef(bondEventId),
          } satisfies ForumWorkRequestOfferProviderBond
        })()

  return {
    amountSats: wholeSatsFromMsats(quote.amountMsats),
    capabilityRefs: quote.capabilityRefs,
    providerActorRef: quote.providerRef,
    providerBond,
    providerPubkey: quotePubkey,
    quoteRef: quote.quoteRef,
    relayEventRef: forumWorkRequestEventRef(quoteEventId),
  }
}

// Bridge (a): a provider Pylon publishes its kind-7000 quote on the relay,
// then submits the public quote refs here so the requester can see and accept
// the live offer. Registered-agent bearer auth; idempotent on quoteRef.
const submitForumWorkRequestOfferResponse = (
  request: Request,
  db: D1Database,
  workRequestId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const body = yield* decodeJsonBody(
      request,
      decodeSubmitForumWorkRequestOfferBody,
    )
    const workRequest = yield* readForumWorkRequestById(db, workRequestId)

    if (workRequest === null) {
      return notFound()
    }

    // Authenticate the submitting provider as a registered agent, mirroring
    // the other forum-work-request write routes.
    yield* actorForRequest(request, dependencies)

    const existing = yield* readForumWorkRequestOfferByQuoteRef(
      db,
      workRequestId,
      body.quoteRef,
    )

    if (existing !== null) {
      return noStoreJsonResponse({ idempotent: true, offer: existing })
    }

    const makeId = dependencies.makeId ?? randomUuid
    const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
    const offer = yield* recordForumWorkRequestOffer(
      db,
      {
        amountSats: body.amountSats,
        capabilityRefs:
          body.capabilityRefs === undefined ||
          body.capabilityRefs.length === 0
            ? workRequest.requiredCapabilityRefs
            : body.capabilityRefs,
        offerId: makeId(),
        providerActorRef: body.providerActorRef,
        providerPubkey: body.providerPubkey ?? null,
        quoteRef: body.quoteRef,
        relayEventRef: body.relayEventRef ?? null,
        workRequestId,
      },
      nowIso,
    )

    return noStoreJsonResponse({ idempotent: false, offer }, { status: 201 })
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

// Bridge (a2): bridge-held relay ingestion path. The caller submits the
// provider's public kind-7000 quote event (and optional provider-bond event),
// this route decodes the NIP-LBR refs and records the same API offer shape as
// the manual bridge above.
const ingestRelayNativeForumWorkRequestOfferResponse = (
  request: Request,
  db: D1Database,
  workRequestId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const body = yield* decodeJsonBody(
      request,
      decodeRelayNativeForumWorkRequestOfferBody,
    )
    const workRequest = yield* readForumWorkRequestById(db, workRequestId)

    if (workRequest === null) {
      return notFound()
    }

    // Registered-agent auth mirrors the manual offer bridge and keeps relay
    // ingestion a server-mediated public-ref write, not an unauthenticated
    // mutation path.
    yield* actorForRequest(request, dependencies)

    const decoded = yield* Effect.try({
      catch: error =>
        error instanceof ForumValidationError
          ? error
          : new ForumValidationError({
              reason:
                error instanceof Error
                  ? error.message
                  : 'Relay offer event is invalid.',
            }),
      try: () => decodeRelayOfferForWorkRequest(workRequest, body),
    })

    const existing = yield* readForumWorkRequestOfferByQuoteRef(
      db,
      workRequestId,
      decoded.quoteRef,
    )

    if (existing !== null) {
      return noStoreJsonResponse({ idempotent: true, offer: existing })
    }

    const makeId = dependencies.makeId ?? randomUuid
    const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
    const offer = yield* recordForumWorkRequestOffer(
      db,
      {
        amountSats: decoded.amountSats,
        capabilityRefs: decoded.capabilityRefs,
        offerId: makeId(),
        providerActorRef: decoded.providerActorRef,
        providerBond: decoded.providerBond,
        providerPubkey: decoded.providerPubkey,
        quoteRef: decoded.quoteRef,
        relayEventRef: decoded.relayEventRef,
        workRequestId,
      },
      nowIso,
    )

    return noStoreJsonResponse({ idempotent: false, offer }, { status: 201 })
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

// Bridge (b): publishes the requester-side kind-7000 acceptance feedback event
// to the scoped market relay so a watching provider executes. Best-effort and
// non-fatal: the DB acceptance + escrow reserve are already committed, so a
// relay miss returns a public-safe failure slug rather than failing the
// acceptance. Only public refs cross the boundary.
const publishForumWorkRequestAcceptanceToRelay = (
  input: Readonly<{
    acceptanceRef: string
    escrowReceiptRef: string
    jobEventId: string
    providerPubkey: string | null
    quoteRef: string
    relayUrl: string
    workRequestId: string
  }>,
  dependencies: ForumRouteDependencies,
): Effect.Effect<
  Readonly<{
    accepted: boolean
    acceptanceEventId: string | null
    relayRef: string | null
    reason?: string
  }>,
  never
> =>
  Effect.gen(function* () {
    const publisher = dependencies.forumWorkRequestRelayPublisher

    if (publisher?.publishAcceptance === undefined) {
      return {
        accepted: false,
        acceptanceEventId: null,
        reason: 'relay_publisher_unconfigured',
        relayRef: null,
      }
    }

    if (input.providerPubkey === null) {
      return {
        accepted: false,
        acceptanceEventId: null,
        reason: 'provider_pubkey_missing',
        relayRef: null,
      }
    }

    const providerPubkey = input.providerPubkey

    const receipt = yield* Effect.tryPromise({
      catch: error =>
        new ForumStorageError({
          operation: 'forumWorkRequests.publishAcceptanceRelay',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        publisher.publishAcceptance!({
          acceptanceRef: input.acceptanceRef,
          escrowReceiptRef: input.escrowReceiptRef,
          jobEventId: input.jobEventId,
          providerPubkey,
          quoteRef: input.quoteRef,
          relayUrl: input.relayUrl,
          workRequestId: input.workRequestId,
        }),
    })

    return {
      accepted: receipt.accepted,
      acceptanceEventId: receipt.acceptanceEventId,
      relayRef: receipt.relayRef,
    }
  }).pipe(
    Effect.catch(() =>
      Effect.succeed({
        accepted: false,
        acceptanceEventId: null,
        reason: 'relay_publish_failed',
        relayRef: null,
      }),
    ),
  )

const reserveForumWorkRequestAcceptanceEscrow = (
  db: D1Database,
  input: ReserveLaborEscrowInput,
  dependencies: ForumRouteDependencies,
): Effect.Effect<ForumWorkRequestEscrowReserveResult, ForumStorageError> =>
  Effect.tryPromise({
    catch: error =>
      new ForumStorageError({
        operation: 'forumWorkRequests.reserveAcceptanceEscrow',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () => {
      if (dependencies.forumWorkRequestEscrowReserver !== undefined) {
        return dependencies.forumWorkRequestEscrowReserver(input, db)
      }

      try {
        const result = await reserveLaborEscrow(
          dependencies.treasuryDb ?? db,
          input,
        )

        if (result.kind === 'ok') {
          return {
            escrow: result.escrow,
            ok: true,
            reserveReceiptRef: result.escrow.reserveReceiptRef,
          }
        }

        return result.availableMsat === undefined
          ? { ok: false, reason: result.reason }
          : {
              availableMsat: result.availableMsat,
              ok: false,
              reason: result.reason,
            }
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error)

        if (
          reason.includes('UNIQUE') ||
          reason.includes('labor_escrows.work_request_id')
        ) {
          return { ok: false, reason: 'quote_already_accepted' }
        }

        throw error
      }
    },
  })

const acceptForumWorkRequestOfferResponse = (
  request: Request,
  db: D1Database,
  workRequestId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const body = yield* decodeJsonBody(
      request,
      decodeAcceptForumWorkRequestOfferBody,
    )
    const workRequest = yield* readForumWorkRequestById(db, workRequestId)

    if (workRequest === null) {
      return notFound()
    }

    const actor = yield* actorForRequest(request, dependencies)
    const actorRef = actorRefForForumActor(actor)

    if (actorRef !== workRequest.requesterActorRef) {
      return forbidden('only the requester can accept a work quote')
    }

    const existingByKey = yield* readForumWorkRequestAcceptanceByIdempotencyKey(
      db,
      idempotencyKey,
    )

    if (existingByKey !== null) {
      if (
        existingByKey.workRequestId !== workRequestId ||
        existingByKey.quoteRef !== body.quoteRef
      ) {
        return idempotencyConflictResponse()
      }

      const [offer, offers, relayLink] = yield* Effect.all([
        readForumWorkRequestOfferByQuoteRef(db, workRequestId, body.quoteRef),
        listForumWorkRequestOffers(db, workRequestId),
        readForumWorkRequestRelayLinkByWorkRequestId(db, workRequestId),
      ])

      if (offer === null) {
        return serverError()
      }

      return noStoreJsonResponse({
        ...forumWorkRequestStatusEnvelope({
          acceptance: existingByKey,
          offers,
          relayLink,
          workRequest,
        }),
        acceptedOffer: offer,
        idempotent: true,
      })
    }

    const [offer, existingByRequest] = yield* Effect.all([
      readForumWorkRequestOfferByQuoteRef(db, workRequestId, body.quoteRef),
      readForumWorkRequestAcceptanceByWorkRequestId(db, workRequestId),
    ])

    if (offer === null) {
      return notFound()
    }

    if (existingByRequest !== null) {
      return noStoreJsonResponse(
        {
          acceptedQuoteRef: existingByRequest.quoteRef,
          error: 'quote_already_accepted',
          reason: 'This work request already has an accepted quote.',
        },
        { status: 409 },
      )
    }

    if (
      workRequest.state !== 'open' &&
      workRequest.state !== 'quote_received'
    ) {
      return noStoreJsonResponse(
        {
          error: 'work_request_not_accepting_quotes',
          reason: `Work request is ${workRequest.state}.`,
        },
        { status: 409 },
      )
    }

    if (offer.state !== 'offered') {
      return noStoreJsonResponse(
        {
          error: 'quote_not_acceptable',
          reason: `Quote is ${offer.state}.`,
        },
        { status: 409 },
      )
    }

    if (offer.amountMsats > workRequest.budgetMsats) {
      return noStoreJsonResponse(
        {
          error: 'quote_exceeds_budget',
          reason: 'Quote amount exceeds the work request budget.',
        },
        { status: 409 },
      )
    }

    const makeId = dependencies.makeId ?? randomUuid
    const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
    const acceptanceId = makeId()
    const escrowId = makeId()
    const reserveReceiptId = makeId()
    const reserveReceiptRef = workRequestReserveReceiptRef(
      workRequestId,
      body.quoteRef,
    )
    const acceptanceEventRef = workRequestAcceptanceEventRef(
      workRequestId,
      body.quoteRef,
    )
    const reserve = yield* reserveForumWorkRequestAcceptanceEscrow(
      db,
      {
        amountMsat: offer.amountMsats,
        escrowId,
        fundingSource: { kind: 'ledger_balance' },
        idempotencyKey,
        jobEventId: workRequest.jobEventId,
        nowIso,
        requesterActorRef: workRequest.requesterActorRef,
        reserveReceiptId,
        reserveReceiptRef,
        workRequestId,
      },
      dependencies,
    )

    if (!reserve.ok) {
      return noStoreJsonResponse(
        {
          availableMsat: reserve.availableMsat,
          error: 'labor_escrow_refused',
          reason: reserve.reason,
        },
        { status: 409 },
      )
    }

    const acceptance = yield* recordForumWorkRequestAcceptance(db, {
      acceptanceEventRef,
      acceptanceId,
      amountMsats: offer.amountMsats,
      escrowId,
      idempotencyKey,
      nowIso,
      offerId: offer.offerId,
      providerActorRef: offer.providerActorRef,
      quoteRef: offer.quoteRef,
      requesterActorRef: workRequest.requesterActorRef,
      reserveReceiptRef: reserve.reserveReceiptRef,
      workRequestId,
    })
    const acceptanceRelay = yield* publishForumWorkRequestAcceptanceToRelay(
      {
        acceptanceRef: workRequestAcceptanceRef(workRequestId, offer.quoteRef),
        escrowReceiptRef: reserve.reserveReceiptRef,
        jobEventId: workRequest.jobEventId,
        providerPubkey: offer.providerPubkey,
        quoteRef: offer.quoteRef,
        relayUrl: workRequest.relayUrl,
        workRequestId,
      },
      dependencies,
    )
    const [updated, offers, relayLink] = yield* Effect.all([
      readForumWorkRequestById(db, workRequestId),
      listForumWorkRequestOffers(db, workRequestId),
      readForumWorkRequestRelayLinkByWorkRequestId(db, workRequestId),
    ])

    if (updated === null) {
      return serverError()
    }

    return noStoreJsonResponse(
      {
        ...forumWorkRequestStatusEnvelope({
          acceptance,
          offers,
          relayLink,
          workRequest: updated,
        }),
        acceptanceRelay,
        acceptedOffer: offer,
        idempotent: false,
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

// Bridge (c): the provider publishes its kind-6934 result on the relay, then
// records the delivered result here against the accepted offer. Registered-
// agent bearer auth; idempotent on quoteRef; public refs only.
const submitForumWorkRequestResultResponse = (
  request: Request,
  db: D1Database,
  workRequestId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const body = yield* decodeJsonBody(
      request,
      decodeSubmitForumWorkRequestResultBody,
    )
    const workRequest = yield* readForumWorkRequestById(db, workRequestId)

    if (workRequest === null) {
      return notFound()
    }

    yield* actorForRequest(request, dependencies)

    const [offer, acceptance, existing] = yield* Effect.all([
      readForumWorkRequestOfferByQuoteRef(db, workRequestId, body.quoteRef),
      readForumWorkRequestAcceptanceByWorkRequestId(db, workRequestId),
      readForumWorkRequestResultByQuoteRef(db, workRequestId, body.quoteRef),
    ])

    if (existing !== null) {
      return noStoreJsonResponse({ idempotent: true, result: existing })
    }

    if (offer === null) {
      return notFound()
    }

    if (acceptance === null || acceptance.quoteRef !== body.quoteRef) {
      return noStoreJsonResponse(
        {
          error: 'result_requires_accepted_offer',
          reason: 'A result can only be recorded against the accepted quote.',
        },
        { status: 409 },
      )
    }

    const makeId = dependencies.makeId ?? randomUuid
    const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
    const result = yield* recordForumWorkRequestResult(
      db,
      {
        artifactRefs: body.artifactRefs,
        closeoutRef: body.closeoutRef ?? null,
        offerId: offer.offerId,
        providerActorRef: offer.providerActorRef,
        quoteRef: body.quoteRef,
        resultEventRef: body.resultEventRef,
        resultId: makeId(),
        verificationCommandRef: body.verificationCommandRef,
        workRequestId,
      },
      nowIso,
    )

    return noStoreJsonResponse({ idempotent: false, result }, { status: 201 })
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const releaseForumWorkRequestEscrow = (
  db: D1Database,
  input: import('./labor-escrow').ReleaseLaborEscrowInput,
  dependencies: ForumRouteDependencies,
): Effect.Effect<
  | Readonly<{ ok: true; escrow: LaborEscrowRecord; idempotent: boolean }>
  | Readonly<{ ok: false; reason: string; currentState?: string }>,
  ForumStorageError
> =>
  Effect.tryPromise({
    catch: error =>
      new ForumStorageError({
        operation: 'forumWorkRequests.releaseAcceptanceEscrow',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () => {
      const result = await releaseLaborEscrow(
        dependencies.treasuryDb ?? db,
        input,
      )

      if (result.kind === 'ok') {
        return {
          escrow: result.escrow,
          idempotent: result.idempotent,
          ok: true as const,
        }
      }

      return result.currentState === undefined
        ? { ok: false as const, reason: result.reason }
        : { currentState: result.currentState, ok: false as const, reason: result.reason }
    },
  })

// Bridge (d): a validator-pass release that moves the reserved escrow to the
// provider balance exactly once and records a public release receipt ref.
// Only the requester (release authority) may trigger it; release requires the
// recorded result and a public verification verdict ref as acceptance
// evidence. Idempotent: a second call after release returns the released
// escrow without moving funds again.
const releaseForumWorkRequestEscrowResponse = (
  request: Request,
  db: D1Database,
  workRequestId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const body = yield* decodeJsonBody(
      request,
      decodeReleaseForumWorkRequestBody,
    )
    const workRequest = yield* readForumWorkRequestById(db, workRequestId)

    if (workRequest === null) {
      return notFound()
    }

    const actor = yield* actorForRequest(request, dependencies)
    const actorRef = actorRefForForumActor(actor)

    if (actorRef !== workRequest.requesterActorRef) {
      return forbidden('only the requester can release a work escrow')
    }

    const [acceptance, result] = yield* Effect.all([
      readForumWorkRequestAcceptanceByWorkRequestId(db, workRequestId),
      readForumWorkRequestResultByQuoteRef(db, workRequestId, body.quoteRef),
    ])

    if (acceptance === null || acceptance.quoteRef !== body.quoteRef) {
      return noStoreJsonResponse(
        {
          error: 'release_requires_accepted_offer',
          reason: 'Escrow release requires the accepted quote.',
        },
        { status: 409 },
      )
    }

    if (result === null) {
      return noStoreJsonResponse(
        {
          error: 'release_requires_recorded_result',
          reason: 'Record the delivered result before releasing escrow.',
        },
        { status: 409 },
      )
    }

    const escrowBefore = yield* Effect.tryPromise({
      catch: error =>
        new ForumStorageError({
          operation: 'forumWorkRequests.readEscrowForRelease',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => readLaborEscrowById(db, acceptance.escrowId),
    })

    if (escrowBefore === null) {
      return serverError()
    }

    const makeId = dependencies.makeId ?? randomUuid
    const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
    const release = yield* releaseForumWorkRequestEscrow(
      db,
      {
        acceptanceEventRef: body.verificationVerdictRef,
        authority: {
          actorRef: workRequest.requesterActorRef,
          kind: 'requester_acceptance',
        },
        escrowId: acceptance.escrowId,
        nowIso,
        providerActorRef: acceptance.providerActorRef,
        releaseReceiptId: makeId(),
        releaseReceiptRef: workRequestReleaseReceiptRef(
          workRequestId,
          body.quoteRef,
        ),
      },
      dependencies,
    )

    if (!release.ok) {
      // Exactly-once: a prior release already moved the funds. Surface the
      // already-released escrow as an idempotent success rather than refusing.
      if (
        release.reason === 'escrow_not_reserved' &&
        escrowBefore.state === 'released_to_provider'
      ) {
        yield* markForumWorkRequestSettled(db, workRequestId, nowIso)
        return noStoreJsonResponse({
          escrow: escrowBefore,
          idempotent: true,
          released: true,
          result,
        })
      }

      return noStoreJsonResponse(
        {
          currentState: release.currentState,
          error: 'labor_escrow_release_refused',
          reason: release.reason,
        },
        { status: 409 },
      )
    }

    // Escrow moved to the provider — advance the request to terminal `settled`
    // so the public projection and lifecycle reflect the closed-out job.
    yield* markForumWorkRequestSettled(db, workRequestId, nowIso)

    return noStoreJsonResponse({
      escrow: release.escrow,
      idempotent: release.idempotent,
      released: true,
      result,
    })
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const createForumWorkRequestLifecycleResponse = (
  request: Request,
  db: D1Database,
  workRequestId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const body = yield* decodeJsonBody(
      request,
      decodeForumWorkRequestLifecycleBody,
    )
    const existingLifecycle =
      yield* readForumWorkRequestLifecycleByIdempotencyKey(db, idempotencyKey)

    if (existingLifecycle !== null) {
      if (
        existingLifecycle.workRequestId !== workRequestId ||
        existingLifecycle.lifecycleKind !== body.lifecycleKind ||
        existingLifecycle.receiptRef !== body.receiptRef
      ) {
        return idempotencyConflictResponse()
      }

      const [workRequest, post] = yield* Effect.all([
        readForumWorkRequestById(db, workRequestId),
        readForumPostById(db, existingLifecycle.postId),
      ])

      if (workRequest === null || post === null) {
        return serverError()
      }

      return noStoreJsonResponse({
        idempotent: true,
        lifecyclePost: existingLifecycle,
        post,
        workRequest,
      })
    }

    const workRequest = yield* readForumWorkRequestById(db, workRequestId)

    if (workRequest === null) {
      return notFound()
    }

    const topic = yield* readForumTopicById(db, workRequest.topicId)

    if (
      topic === null ||
      topic.state === 'archived' ||
      topic.state === 'hidden'
    ) {
      return notFound()
    }

    if (topic.state === 'locked') {
      return locked('topic is locked')
    }

    const forum = yield* readForumSummaryByRef(db, topic.forumId, {
      allowUnlisted: true,
    })

    if (forum === null) {
      return notFound()
    }

    const bodyText = yield* Effect.try({
      catch: forumWorkRequestErrorToValidationError,
      try: () =>
        forumWorkRequestLifecycleBodyText(
          body.lifecycleKind,
          body.receiptRef,
          workRequestId,
        ),
    })
    const actor = yield* actorForRequest(request, dependencies)
    const nowEpochMillis = dependencies.nowEpochMillis ?? currentEpochMillis
    const publicIdentity = yield* verifiedPublicIdentityForActor(
      actor,
      dependencies,
    )
    const grant = forumWriteGrantForActor(
      actor,
      forum.forumId,
      forumWriteRequiredScopeForForum(forum.slug),
      nowEpochMillis,
      publicIdentity,
    )
    const writer = yield* buildForumWriterContext({
      actor,
      grant,
      nowEpochMillis,
      paymentProofRef: null,
      requiredScope: forumWriteRequiredScopeForForum(forum.slug),
      targetForumId: forum.forumId,
      targetOwnerUserId:
        actor._tag === 'Agent' ? (publicIdentity?.ownerUserId ?? null) : null,
      targetTeamId: null,
    })
    const writePolicyDenial = yield* enforceForumWritePolicy(db, {
      actionKind: 'reply',
      actorRef: writer.actor.actorRef,
      bodyText,
      nowEpochMillis: nowEpochMillis(),
    })

    if (writePolicyDenial !== null) {
      return writePolicyDenial
    }

    const makeId = dependencies.makeId ?? randomUuid
    const postId = makeId()
    const runtime = {
      makeId,
      nowIso: dependencies.nowIso ?? currentIsoTimestamp,
    }
    const post = yield* createForumReplyPost(
      db,
      {
        actor: writer.actor,
        bodyText,
        contentRef: `content.forum.work_request_lifecycle.${postId}`,
        forumId: forum.forumId,
        idempotencyKey,
        parentPostId: topic.latestPostId,
        postId,
        publicProjection: defaultPublicProjection(
          `artifact.forum.work_request_lifecycle.${postId}`,
        ),
        quotePostId: null,
        topicId: topic.topicId,
      },
      runtime,
    )
    const lifecyclePost = yield* recordForumWorkRequestLifecyclePost(
      db,
      {
        idempotencyKey,
        lifecycleKind: body.lifecycleKind,
        lifecyclePostId: makeId(),
        postId,
        receiptRef: body.receiptRef,
        topicId: topic.topicId,
        workRequestId,
      },
      runtime,
    )
    const updated = yield* readForumWorkRequestById(db, workRequestId)

    if (updated === null) {
      return serverError()
    }

    return noStoreJsonResponse(
      {
        idempotent: false,
        lifecyclePost,
        post,
        workRequest: updated,
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const writerForForumResponse = (
  request: Request,
  dependencies: ForumRouteDependencies,
  input: Readonly<{
    forumId: string
    forumSlug: string
  }>,
) =>
  Effect.gen(function* () {
    const actor = yield* actorForRequest(request, dependencies)
    const nowEpochMillis = dependencies.nowEpochMillis ?? currentEpochMillis
    const requiredScope = forumWriteRequiredScopeForForum(input.forumSlug)
    const publicIdentity = yield* verifiedPublicIdentityForActor(
      actor,
      dependencies,
    )
    const grant = forumWriteGrantForActor(
      actor,
      input.forumId,
      requiredScope,
      nowEpochMillis,
      publicIdentity,
    )

    return yield* buildForumWriterContext({
      actor,
      grant,
      nowEpochMillis,
      paymentProofRef: null,
      requiredScope,
      targetForumId: input.forumId,
      targetOwnerUserId:
        actor._tag === 'Agent' ? (publicIdentity?.ownerUserId ?? null) : null,
      targetTeamId: null,
    })
  })

const readPostControlTarget = (db: D1Database, postId: string) =>
  Effect.gen(function* () {
    const postDetail = yield* readForumPostDetail(db, postId)

    if (postDetail === null) {
      return null
    }

    const topic = yield* readForumTopicById(db, postDetail.containingTopicId)

    if (
      topic === null ||
      topic.state === 'hidden' ||
      topic.state === 'archived'
    ) {
      return null
    }

    const forum = yield* readForumSummaryByRef(db, topic.forumId, {
      allowUnlisted: true,
    })

    if (forum === null) {
      return null
    }

    return { forum, postDetail, topic }
  })

const editPostResponse = (
  request: Request,
  db: D1Database,
  postId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const existingRevision = yield* readForumPostRevisionByIdempotencyKey(
      db,
      idempotencyKey,
    )

    if (existingRevision !== null) {
      const existingPost = yield* readForumPostById(
        db,
        existingRevision.post_id,
      )

      return existingPost === null
        ? serverError()
        : noStoreJsonResponse(
            revisionResultFromRow(existingRevision, existingPost, true),
          )
    }

    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(EditForumPostBody),
    )
    const target = yield* readPostControlTarget(db, postId)

    if (target === null) {
      return notFound()
    }

    if (target.topic.state === 'locked') {
      return locked('topic is locked')
    }

    if (target.forum.locked) {
      return locked('forum is locked')
    }

    if (
      target.postDetail.post.state === 'hidden' ||
      target.postDetail.post.state === 'held_for_review' ||
      target.postDetail.post.state === 'tombstoned'
    ) {
      return notFound()
    }

    const writer = yield* writerForForumResponse(request, dependencies, {
      forumId: target.forum.forumId,
      forumSlug: target.forum.slug,
    })

    if (writer.actor.actorRef !== target.postDetail.post.author.actorRef) {
      return forbidden('only the post author can edit this post')
    }

    // Authors may repair threading: a supplied parentPostId is honored with
    // the same validation as reply creation plus a bounded cycle guard, and
    // an explicit null re-parents the post to top level (#4856).
    const requestedParentPostId = body.parentPostId

    if (requestedParentPostId !== undefined && requestedParentPostId !== null) {
      if (requestedParentPostId === target.postDetail.post.postId) {
        return badRequest('parentPostId must not reference the edited post')
      }

      const parentRef = yield* readForumPostThreadRef(
        db,
        requestedParentPostId,
      )
      const parentDenialReason = invalidForumReplyParentPostReference(
        parentRef,
        target.topic.topicId,
      )

      if (parentDenialReason !== null) {
        return badRequest(parentDenialReason)
      }

      const wouldCycle = yield* forumPostThreadHasAncestor(db, {
        ancestorPostId: target.postDetail.post.postId,
        startPostId: requestedParentPostId,
      })

      if (wouldCycle) {
        return badRequest('parentPostId must not create a reply cycle')
      }
    }

    const makeId = dependencies.makeId ?? randomUuid
    const revisionId = makeId()
    const post = yield* editForumPostBody(db, {
      actorRef: writer.actor.actorRef,
      id: revisionId,
      idempotencyKey,
      nextBodyText: body.bodyText,
      nextParentPostId: requestedParentPostId,
      postId: target.postDetail.post.postId,
      publicProjection: defaultPublicProjection(
        `artifact.forum.post_revision.${revisionId}`,
      ),
      reasonRef: null,
    })

    return noStoreJsonResponse(
      {
        action: 'edit',
        idempotent: false,
        post,
        revisionRef: post.revisionRef ?? revisionId,
      },
      { status: 200 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const readTopicControlTarget = (db: D1Database, topicId: string) =>
  Effect.gen(function* () {
    const topic = yield* readForumTopicById(db, topicId)

    if (
      topic === null ||
      topic.state === 'hidden' ||
      topic.state === 'archived'
    ) {
      return null
    }

    const forum = yield* readForumSummaryByRef(db, topic.forumId, {
      allowUnlisted: true,
    })

    if (forum === null) {
      return null
    }

    return { forum, topic }
  })

const editTopicResponse = (
  request: Request,
  db: D1Database,
  topicId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(EditForumTopicBody),
    )
    const target = yield* readTopicControlTarget(db, topicId)

    if (target === null) {
      return notFound()
    }

    if (target.topic.state === 'locked') {
      return locked('topic is locked')
    }

    if (target.forum.locked) {
      return locked('forum is locked')
    }

    const writer = yield* writerForForumResponse(request, dependencies, {
      forumId: target.forum.forumId,
      forumSlug: target.forum.slug,
    })

    if (writer.actor.actorRef !== target.topic.author.actorRef) {
      return forbidden('only the topic author can rename this topic')
    }

    const topic = yield* updateForumTopicTitle(db, {
      title: body.title,
      topicId: target.topic.topicId,
    })

    if (topic === null) {
      return notFound()
    }

    return noStoreJsonResponse(
      {
        action: 'rename',
        idempotent: false,
        topic,
      },
      { status: 200 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const tombstonePostResponse = (
  request: Request,
  db: D1Database,
  postId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const existingRevision = yield* readForumPostRevisionByIdempotencyKey(
      db,
      idempotencyKey,
    )

    if (existingRevision !== null) {
      const existingPost = yield* readForumPostById(
        db,
        existingRevision.post_id,
      )

      return existingPost === null
        ? serverError()
        : noStoreJsonResponse(
            revisionResultFromRow(existingRevision, existingPost, true),
          )
    }

    const body = yield* decodeJsonBodyOrDefault(
      request,
      S.decodeUnknownSync(TombstoneForumPostBody),
      {},
    )
    const target = yield* readPostControlTarget(db, postId)

    if (target === null) {
      return notFound()
    }

    if (target.topic.state === 'locked') {
      return locked('topic is locked')
    }

    if (target.forum.locked) {
      return locked('forum is locked')
    }

    if (
      target.postDetail.post.state === 'hidden' ||
      target.postDetail.post.state === 'held_for_review' ||
      target.postDetail.post.state === 'tombstoned'
    ) {
      return notFound()
    }

    const writer = yield* writerForForumResponse(request, dependencies, {
      forumId: target.forum.forumId,
      forumSlug: target.forum.slug,
    })

    if (writer.actor.actorRef !== target.postDetail.post.author.actorRef) {
      return forbidden('only the post author can tombstone this post')
    }

    const makeId = dependencies.makeId ?? randomUuid
    const revisionId = makeId()
    const reason = body.reason ?? 'author_request'
    const post = yield* tombstoneForumPost(db, {
      actorRef: writer.actor.actorRef,
      id: revisionId,
      idempotencyKey,
      postId: target.postDetail.post.postId,
      publicProjection: defaultPublicProjection(
        `artifact.forum.post_revision.${revisionId}`,
      ),
      reasonRef: `forum.post.tombstone.${reason}`,
    })

    return noStoreJsonResponse({
      action: 'tombstone',
      idempotent: false,
      post,
      revisionRef: post.revisionRef ?? revisionId,
    })
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const reportForumTargetResponse = (
  request: Request,
  db: D1Database,
  target: Readonly<{
    forumId: string
    forumSlug: string
    targetId: string
    targetKind: 'topic' | 'post'
  }>,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const existingReport = yield* readForumReportByIdempotencyKey(
      db,
      idempotencyKey,
    )

    if (existingReport !== null) {
      return noStoreJsonResponse(reportResultFromRow(existingReport, true))
    }

    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(ReportForumTargetBody),
    )
    const writer = yield* writerForForumResponse(request, dependencies, {
      forumId: target.forumId,
      forumSlug: target.forumSlug,
    })
    const makeId = dependencies.makeId ?? randomUuid
    const reportId = makeId()
    const reasonRef = ForumReportReasonRefs[body.reason]

    yield* recordForumReport(db, {
      id: reportId,
      idempotencyKey,
      publicProjection: defaultPublicProjection(
        `artifact.forum.report.${reportId}`,
      ),
      reasonRef,
      reporterActorRef: writer.actor.actorRef,
      targetId: target.targetId,
      targetKind: target.targetKind,
    })

    return noStoreJsonResponse(
      {
        idempotent: false,
        report: {
          reason: body.reason,
          reportId,
          status: 'open',
          targetId: target.targetId,
          targetKind: target.targetKind,
        },
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const moderationQueueResponse = (
  request: Request,
  db: D1Database,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    yield* moderatorForRequest(request, dependencies)

    const url = new URL(request.url)
    const limit = forumListLimitFromUrl(url)

    if (limit instanceof Response) {
      return limit
    }

    const items = yield* listForumModerationQueue(db, { limit })

    return noStoreJsonResponse({
      generatedAt: dependencies.nowIso?.() ?? currentIsoTimestamp(),
      items,
      pagination: {
        cursor: null,
        hasMore: false,
        limit,
        nextCursor: null,
      },
    })
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const moderationItemResponse = (
  request: Request,
  db: D1Database,
  input: Readonly<{
    itemId: string
    itemKind: 'report' | 'post_review' | 'topic_review'
  }>,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    yield* moderatorForRequest(request, dependencies)

    const item = yield* readForumModerationItem(db, input)

    return item === null ? notFound() : noStoreJsonResponse(item)
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const moderationActionResponse = (
  request: Request,
  db: D1Database,
  input: Readonly<{
    actionKind: string
    reportId?: string | null
    targetId: string
    targetKind: 'post' | 'topic' | 'report'
    update: () => Effect.Effect<unknown | null, ForumStorageError>
  }>,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const existingEvent = yield* readForumModerationEventByIdempotencyKey(
      db,
      idempotencyKey,
    )

    if (existingEvent !== null) {
      return noStoreJsonResponse(
        moderationEventResultFromRow(existingEvent, true),
      )
    }

    const moderator = yield* moderatorForRequest(request, dependencies)
    const body = yield* decodeJsonBodyOrDefault(
      request,
      S.decodeUnknownSync(ForumModerationActionBody),
      {},
    )
    const target = yield* input.update()

    if (target === null) {
      return notFound()
    }

    const makeId = dependencies.makeId ?? randomUuid
    const eventId = makeId()
    const reason = body.reason ?? 'policy_reviewed'
    yield* recordForumModerationEvent(db, {
      actionKind: input.actionKind,
      id: eventId,
      idempotencyKey,
      moderatorActorRef: `operator:${moderator.operatorId}`,
      publicProjection: defaultPublicProjection(
        `artifact.forum.moderation_event.${eventId}`,
      ),
      reasonRef: ForumModerationReasonRefs[reason],
      reportId: input.reportId ?? null,
      targetId: input.targetId,
      targetKind: input.targetKind,
    })

    return noStoreJsonResponse(
      {
        idempotent: false,
        moderationEvent: {
          actionKind: input.actionKind,
          eventId,
          reasonRef: ForumModerationReasonRefs[reason],
          reportId: input.reportId ?? null,
          targetId: input.targetId,
          targetKind: input.targetKind,
        },
        target,
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const tipRecipientAdmissionResponse = (
  request: Request,
  db: D1Database,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const moderator = yield* moderatorForRequest(request, dependencies)
    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(ForumTipRecipientAdmissionBody),
    )
    const nowIso = dependencies.nowIso ?? currentIsoTimestamp
    const walletId = `forum_tip_recipient_wallet.${refIdSegment(
      body.actorRef,
      'actor',
    )}`
    const disabledAt =
      body.state === 'ready' ? null : (body.disabledAt ?? nowIso())
    const tipRecipientReadiness = yield* upsertForumTipRecipientWallet(
      dependencies.treasuryDb ?? db,
      {
        actorRef: body.actorRef,
        sparkAddress: body.sparkAddress ?? null,
        bolt12Offer: body.bolt12Offer ?? null,
        lightningAddress: body.lightningAddress ?? null,
        caveatRefs: body.caveatRefs ?? [],
        claimPolicyRefs: body.claimPolicyRefs ?? [],
        custodyPolicyRefs: body.custodyPolicyRefs ?? [],
        disabledAt,
        id: walletId,
        payoutTargetApprovalRef: body.payoutTargetApprovalRef ?? null,
        providerClass: body.providerClass,
        readinessRefs: body.readinessRefs ?? [],
        receiveCapabilityRef: body.receiveCapabilityRef,
        sourceRef: body.sourceRef,
        state: body.state,
        walletRef: body.walletRef,
      },
      { makeId: dependencies.makeId ?? randomUuid, nowIso },
    )

    return noStoreJsonResponse(
      {
        idempotencyKey,
        idempotent: false,
        moderatorActorRef: `operator:${moderator.operatorId}`,
        tipRecipientReadiness,
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const tipRecipientWalletClaimResponse = (
  request: Request,
  db: D1Database,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const actor = yield* agentForRequest(request, dependencies)
    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(ForumTipRecipientClaimBody),
    )
    const nowIso = dependencies.nowIso ?? currentIsoTimestamp
    const actorRef = actorRefForForumActor(actor)
    const walletId = `forum_tip_recipient_wallet.${refIdSegment(
      actorRef,
      'agent',
    )}.self_claim`
    const providerClass =
      body.providerClass ??
      (body.lightningAddress !== undefined && body.lightningAddress !== null
        ? 'external_lightning'
        : 'mdk_agent_wallet')
    const baseCustodyPolicyRef =
      providerClass === 'mdk_agent_wallet'
        ? 'policy.public.forum_tip_recipient.self_custody_mdk_agent_wallet'
        : 'policy.public.forum_tip_recipient.spark_self_custody'
    const tipRecipientReadiness = yield* upsertForumTipRecipientWallet(
      dependencies.treasuryDb ?? db,
      {
        actorRef,
        sparkAddress: body.sparkAddress ?? null,
        bolt12Offer: body.bolt12Offer ?? null,
        lightningAddress: body.lightningAddress ?? null,
        caveatRefs: [
          'caveat.public.forum_tip_recipient.creator_settlement_pending',
          ...(body.caveatRefs ?? []),
        ],
        claimPolicyRefs: [
          'policy.public.forum_tip_recipient.agent_self_claimed',
          ...(body.claimPolicyRefs ?? []),
        ],
        custodyPolicyRefs: [
          baseCustodyPolicyRef,
          ...(body.custodyPolicyRefs ?? []),
        ],
        disabledAt: null,
        id: walletId,
        payoutTargetApprovalRef: body.payoutTargetApprovalRef ?? null,
        providerClass,
        readinessRefs: body.readinessRefs ?? [],
        receiveCapabilityRef: body.receiveCapabilityRef,
        sourceRef:
          body.sourceRef ??
          'source.public.forum_tip_recipient.agent_self_claim',
        state: 'ready',
        walletRef: body.walletRef,
      },
      { makeId: dependencies.makeId ?? randomUuid, nowIso },
    )

    return noStoreJsonResponse({ tipRecipientReadiness }, { status: 201 })
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const previewPaidActionResponse = (
  request: Request,
  db: D1Database,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(ForumPaidActionPreviewBody),
    )

    if (body.method !== 'POST') {
      return badRequest('Forum paid actions must bind POST method')
    }

    if (!body.path.startsWith('/api/forum/')) {
      return badRequest('Forum paid action path must be under /api/forum')
    }

    const amountError = forumPaidActionAmountError(body.actionKind, body.amount)
    if (amountError !== undefined) {
      return yield* new ForumValidationError({ reason: amountError })
    }

    const actor = yield* actorForRequest(request, dependencies)
    const resolved = yield* resolveForumPaidActionTarget(
      db,
      body.actionKind,
      body.target,
    )

    if (resolved === null) {
      return notFound()
    }

    const preview = yield* previewForumPaidAction(
      dependencies.treasuryDb ?? db,
      {
      actionKind: body.actionKind,
      actorRef: actorRefForForumActor(actor),
      hostedMdkClient: dependencies.hostedMdkClient,
      idempotencyKey,
      method: body.method,
      nonPayableDenial: resolved.nonPayableDenial,
      path: body.path,
      price: postRewardPriceForBody(body.actionKind, body.amount),
      publicProjection: defaultPublicProjection(
        `artifact.forum.paid_action.${body.actionKind}`,
      ),
      recipientActorRef: resolved.recipientActorRef,
      recipientReadinessRef: resolved.recipientReadinessRef,
      requestBodyDigest: body.requestBodyDigest,
      routeParams: body.routeParams ?? {},
      spendCap: body.spendCap,
      target: resolved.target,
    })

    return noStoreJsonResponse(
      preview,
      preview.challenge?.l402 === null || preview.challenge?.l402 === undefined
        ? undefined
        : {
            headers: {
              'www-authenticate': preview.challenge.l402.wwwAuthenticate,
            },
          },
    )
  }).pipe(
    Effect.catch(error => Effect.succeed(paidActionFailureResponse(error))),
  )

const previewAliasPaidActionResponse = (
  request: Request,
  db: D1Database,
  input: Readonly<{
    actionKind: ForumPaidActionKindType
    routeParams: Readonly<Record<string, string>>
    target: ForumPaidActionTargetType
  }>,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(ForumPaidActionAliasPreviewBody),
    )
    const amountError = forumPaidActionAmountError(
      input.actionKind,
      body.amount,
    )
    if (amountError !== undefined) {
      return yield* new ForumValidationError({ reason: amountError })
    }

    const actor = yield* actorForRequest(request, dependencies)
    const resolved = yield* resolveForumPaidActionTarget(
      db,
      input.actionKind,
      input.target,
    )

    if (resolved === null) {
      return notFound()
    }

    const preview = yield* previewForumPaidAction(
      dependencies.treasuryDb ?? db,
      {
      actionKind: input.actionKind,
      actorRef: actorRefForForumActor(actor),
      hostedMdkClient: dependencies.hostedMdkClient,
      idempotencyKey,
      method: 'POST',
      nonPayableDenial: resolved.nonPayableDenial,
      path: new URL(request.url).pathname,
      price: postRewardPriceForBody(input.actionKind, body.amount),
      publicProjection: defaultPublicProjection(
        `artifact.forum.paid_action.${input.actionKind}`,
      ),
      recipientActorRef: resolved.recipientActorRef,
      recipientReadinessRef: resolved.recipientReadinessRef,
      requestBodyDigest: body.requestBodyDigest,
      routeParams: input.routeParams,
      spendCap: body.spendCap,
      target: resolved.target,
    })

    return noStoreJsonResponse(
      preview,
      preview.challenge?.l402 === null || preview.challenge?.l402 === undefined
        ? undefined
        : {
            headers: {
              'www-authenticate': preview.challenge.l402.wwwAuthenticate,
            },
          },
    )
  }).pipe(
    Effect.catch(error => Effect.succeed(paidActionFailureResponse(error))),
  )

const privatePaidActionPaymentResponse = (
  request: Request,
  db: D1Database,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(ForumPaidActionPrivatePaymentBody),
    )
    const actor = yield* actorForRequest(request, dependencies)
    const signingBoundary =
      dependencies.l402SigningBoundary === undefined
        ? undefined
        : yield* Effect.tryPromise({
            catch: error =>
              new ForumPaidActionError({
                kind: 'payment_verification_failed',
                reason:
                  error instanceof Error
                    ? error.message
                    : 'Forum private L402 signer could not be loaded.',
              }),
            try: dependencies.l402SigningBoundary,
          })

    const response = yield* readForumPaidActionPrivatePayment(db, {
      actorRef: actorRefForForumActor(actor),
      challengeId: body.challengeId,
      hostedMdkClient: dependencies.hostedMdkClient,
      method: body.method,
      path: body.path,
      requestBodyDigest: body.requestBodyDigest,
      routeParams: body.routeParams ?? {},
      signingBoundary: signingBoundary ?? undefined,
      spendCap: body.spendCap,
    })

    return noStoreJsonResponse(response)
  }).pipe(
    Effect.catch(error => Effect.succeed(paidActionFailureResponse(error))),
  )

const redeemPaidActionResponse = (
  request: Request,
  db: D1Database,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(ForumPaidActionRedeemBody),
    )
    const actor = yield* actorForRequest(request, dependencies)
    const challenge = yield* lookupForumPaidActionChallenge(
      db,
      body.challengeId,
    )

    if (challenge === null) {
      return notFound()
    }

    const resolved = yield* resolveForumPaidActionTarget(
      db,
      challenge.actionKind,
      challenge.target,
    )

    if (resolved === null) {
      return notFound()
    }

    const paymentEvent = yield* verifyForumL402PaymentEvent({
      challenge,
      headers: request.headers,
      l402ProofRef: body.l402ProofRef,
      nowIso: dependencies.nowIso?.() ?? currentIsoTimestamp(),
      signingBoundary: dependencies.l402SigningBoundary,
    })

    if (challenge.actionKind === 'orange_check') {
      if (actor._tag !== 'Agent') {
        return noStoreJsonResponse(
          {
            error: 'orange_check_requires_agent',
            reason:
              'Orange check purchases are self-purchases by registered agent tokens.',
          },
          { status: 403 },
        )
      }

      const l402 = challenge.l402

      if (
        l402 === null ||
        l402.checkoutRef === null ||
        dependencies.hostedMdkClient === undefined
      ) {
        return noStoreJsonResponse(
          {
            error: 'orange_check_payment_unverifiable',
            reason:
              'Orange check fulfillment requires a hosted checkout binding.',
          },
          { status: 409 },
        )
      }

      const checkoutStatus = yield* dependencies.hostedMdkClient
        .getCheckoutStatus({
          checkoutRef: l402.checkoutRef,
          environment: l402.environment,
          providerRef: l402.providerRef,
          sandbox: l402.sandbox,
          siteRef: null,
        })
        .pipe(
          Effect.mapError(
            () =>
              new ForumPaidActionError({
                kind: 'payment_verification_failed',
                reason:
                  'Orange check checkout status could not be confirmed with the payment provider.',
              }),
          ),
        )

      if (checkoutStatus.status !== 'payment_received') {
        return noStoreJsonResponse(
          {
            checkoutStatus: checkoutStatus.status,
            error: 'orange_check_payment_not_received',
            reason:
              'Pay the checkout invoice first; fulfillment requires provider payment_received status.',
          },
          { status: 402 },
        )
      }

      const orangeRedemption = yield* redeemForumPaidAction(
        dependencies.treasuryDb ?? db,
        {
        actorRef: actorRefForForumActor(actor),
        challengeId: body.challengeId,
        idempotencyKey,
        l402ProofRef: body.l402ProofRef,
        method: body.method,
        path: body.path,
        paymentEvent,
        recipientActorRef: resolved.recipientActorRef,
        recipientReadinessRef: resolved.recipientReadinessRef,
        requestBodyDigest: body.requestBodyDigest,
        routeParams: body.routeParams ?? {},
      })
      const entitlement = yield* grantOrangeCheckEntitlement(
        db,
        {
          actionRef: `forum_paid_action.orange_check.${challenge.challengeId}`,
          actorRef: actorRefForForumActor(actor),
          agentUserId: actor.session.user.id,
          nowIso: dependencies.nowIso?.() ?? currentIsoTimestamp(),
          paidAmountCents: 500,
          receiptRef: `orange_check_receipt.${challenge.challengeId}`,
        },
        dependencies.entitlementsMirror,
      )

      return noStoreJsonResponse(
        {
          ...orangeRedemption,
          orangeCheck: orangeCheckBadgeProjection(entitlement),
        },
        { status: orangeRedemption.replayed ? 200 : 201 },
      )
    }

    const redemption = yield* redeemForumPaidAction(
      dependencies.treasuryDb ?? db,
      {
      actorRef: actorRefForForumActor(actor),
      challengeId: body.challengeId,
      idempotencyKey,
      l402ProofRef: body.l402ProofRef,
      method: body.method,
      path: body.path,
      paymentEvent,
      recipientActorRef: resolved.recipientActorRef,
      recipientReadinessRef: resolved.recipientReadinessRef,
      requestBodyDigest: body.requestBodyDigest,
      routeParams: body.routeParams ?? {},
    })

    return noStoreJsonResponse(redemption, {
      status: redemption.replayed ? 200 : 201,
    })
  }).pipe(
    Effect.catch(error => Effect.succeed(paidActionFailureResponse(error))),
  )

const submitDirectTipResponse = (
  request: Request,
  db: D1Database,
  postId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const actor = yield* actorForRequest(request, dependencies)
    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(ForumDirectTipSubmitBody),
    )
    const postDetail = yield* readForumPostDetail(db, postId)

    if (postDetail === null) {
      return notFound()
    }

    const response = yield* submitForumDirectTip(
      dependencies.treasuryDb ?? db,
      {
      amount: body.amount,
      idempotencyKey,
      payerActorRef: actorRefForForumActor(actor),
      paymentEvidence: body.paymentEvidence,
      post: {
        authorActorRef: postDetail.post.author.actorRef,
        postId: postDetail.post.postId,
        publicProjection: postDetail.post.publicProjection,
        targetPostPermalink: postDetail.post.permalink ?? null,
        topicId: postDetail.post.topicId,
      },
      recipientReadiness: postDetail.post.tipRecipientReadiness,
    })

    return noStoreJsonResponse(response, {
      status: response.idempotent ? 200 : 201,
    })
  }).pipe(
    Effect.catch(error => Effect.succeed(paidActionFailureResponse(error))),
  )

const tipLadderResponse = (
  request: Request,
  db: D1Database,
  postId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const actor = yield* actorForRequest(request, dependencies)

    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(ForumTipLadderBody),
    )
    const postDetail = yield* readForumPostDetail(db, postId)

    if (postDetail === null) {
      return notFound()
    }

    const readiness = postDetail.post.tipRecipientReadiness
    const recipientHasPaymentDestination =
      readiness.state === 'ready' && readiness.directPayment !== null

    const makeId = dependencies.makeId ?? randomUuid
    const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
    const publicReceiptRef =
      body.publicReceiptRef === undefined
        ? yield* Effect.promise(() =>
            tipLadderReceiptRefFromIdempotencyKey(idempotencyKey),
          )
        : body.publicReceiptRef

    if (!isTipLadderReceiptRef(publicReceiptRef)) {
      return badRequest('publicReceiptRef is malformed')
    }

    const directPayment = readiness.directPayment as {
      sparkAddress?: string
      bolt12Offer?: string
      lightningAddress?: string
      kind?: string
    } | null
    // Native Spark address is the preferred rail (Spark→Spark, 0-fee). The
    // buffer-pay adapter receives the destination string and routes it to a
    // `ReceivePaymentMethod::SparkAddress` transfer; Lightning rails fall back
    // for external Lightning senders.
    const recipientPaymentDestination =
      directPayment?.kind === 'spark_address'
        ? directPayment.sparkAddress
        : directPayment?.kind === 'lightning_address'
        ? directPayment.lightningAddress
        : directPayment?.bolt12Offer
    const tipsBufferPay = dependencies.tipsBufferPay ?? null

    const result = yield* executeTipLadder(db, {
      amountSat: body.amountSat,
      idempotencyKey,
      makeId,
      mirror: dependencies.billingMirror,
      nowIso,
      payFromBuffer: tipsBufferPay,
      postId: postDetail.post.postId,
      publicReceiptRef,
      recipientHasPaymentDestination,
      recipientPaymentDestination: recipientPaymentDestination ?? null,
      recipientRef: postDetail.post.author.actorRef,
      senderRef: actorRefForForumActor(actor),
      tipsBufferConfigured: tipsBufferPay !== null,
    }).pipe(
      Effect.catch((error: TipLadderError) =>
        Effect.succeed({
          kind: 'error' as const,
          reason: error.reason,
        }),
      ),
    )

    if (result.kind === 'error') {
      return noStoreJsonResponse(
        { error: 'tip_ladder_failed', reason: result.reason },
        { status: result.reason === 'ledger_batch_failed' ? 409 : 500 },
      )
    }

    if (result.kind === 'refused') {
      return noStoreJsonResponse(
        {
          error: 'tip_ladder_refused',
          reason: result.reason,
          senderBalanceMsat: result.senderBalanceMsat,
        },
        { status: result.reason === 'insufficient_sender_balance' ? 402 : 400 },
      )
    }

    return noStoreJsonResponse(
      {
        amountSat: result.amountSat,
        ladderReason: result.ladderReason,
        payInId: result.payInId,
        receiptRef: result.receiptRef,
        rung: result.rung,
        senderBalanceMsatAfter: result.senderBalanceMsatAfter,
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const pylonTipLadderResponse = (
  request: Request,
  db: D1Database,
  pylonRef: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const pylonStore = dependencies.pylonApiStore

    if (pylonStore === undefined) {
      return noStoreJsonResponse(
        {
          error: 'pylon_tip_ladder_unavailable',
          reason: 'Pylon tipping is not wired in this deployment.',
        },
        { status: 501 },
      )
    }

    const actor = yield* actorForRequest(request, dependencies)
    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(ForumTipLadderBody),
    )
    const registration = yield* Effect.tryPromise({
      catch: error =>
        new ForumStorageError({
          operation: 'pylonTipLadder.readRegistration',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => pylonStore.readRegistration(pylonRef),
    })

    if (registration === undefined) {
      return notFound()
    }

    const makeId = dependencies.makeId ?? randomUuid
    const nowIso = (dependencies.nowIso ?? currentIsoTimestamp)()
    const publicReceiptRef =
      body.publicReceiptRef === undefined
        ? yield* Effect.promise(() =>
            pylonTipLadderReceiptRefFromIdempotencyKey(idempotencyKey),
          )
        : body.publicReceiptRef

    if (
      !isTipLadderReceiptRef(publicReceiptRef) ||
      !publicReceiptRef.startsWith(PYLON_TIP_LADDER_RECEIPT_REF_PREFIX)
    ) {
      return badRequest('publicReceiptRef is malformed')
    }

    const recipientPaymentDestination =
      dependencies.pylonSparkPayoutTargetStore === undefined
        ? undefined
        : yield* Effect.promise(() =>
            resolveSparkPayoutDestination(
              dependencies.pylonSparkPayoutTargetStore!,
              registration.pylonRef,
              async candidatePylonRef => {
                if (candidatePylonRef === registration.pylonRef) {
                  return registration.ownerAgentUserId
                }

                return (
                  await pylonStore.readRegistration(candidatePylonRef)
                )?.ownerAgentUserId
              },
            ),
          )
    const tipsBufferPay = dependencies.tipsBufferPay ?? null
    const recipientActorRef = `agent:${registration.ownerAgentUserId}`

    const result = yield* executeTipLadder(db, {
      amountSat: body.amountSat,
      contextRef: `pylon.${registration.pylonRef}`,
      directPayoutExternalRef: 'pylon.tip_recipient_claim',
      idempotencyKey,
      makeId,
      mirror: dependencies.billingMirror,
      nowIso,
      payFromBuffer: tipsBufferPay,
      postId: registration.pylonRef,
      publicReceiptRef,
      recipientHasPaymentDestination: recipientPaymentDestination !== undefined,
      recipientPaymentDestination: recipientPaymentDestination ?? null,
      recipientRef: recipientActorRef,
      senderRef: actorRefForForumActor(actor),
      tipsBufferConfigured: tipsBufferPay !== null,
    }).pipe(
      Effect.catch((error: TipLadderError) =>
        Effect.succeed({
          kind: 'error' as const,
          reason: error.reason,
        }),
      ),
    )

    if (result.kind === 'error') {
      return noStoreJsonResponse(
        { error: 'pylon_tip_ladder_failed', reason: result.reason },
        { status: result.reason === 'ledger_batch_failed' ? 409 : 500 },
      )
    }

    if (result.kind === 'refused') {
      return noStoreJsonResponse(
        {
          error: 'pylon_tip_ladder_refused',
          reason: result.reason,
          senderBalanceMsat: result.senderBalanceMsat,
        },
        { status: result.reason === 'insufficient_sender_balance' ? 402 : 400 },
      )
    }

    return noStoreJsonResponse(
      {
        amountSat: result.amountSat,
        ladderReason: result.ladderReason,
        payInId: result.payInId,
        pylonRef: registration.pylonRef,
        receiptRef: result.receiptRef,
        recipientActorRef,
        rung: result.rung,
        senderBalanceMsatAfter: result.senderBalanceMsatAfter,
      },
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const directTipStatusResponse = (db: D1Database, attemptId: string) =>
  lookupForumDirectTip(db, attemptId).pipe(
    Effect.map(response =>
      response === null ? notFound() : noStoreJsonResponse(response),
    ),
    Effect.catch(error => Effect.succeed(paidActionFailureResponse(error))),
  )

const orangeCheckNostrExportResponse = (
  db: D1Database,
  actorRef: string,
  url: URL,
  dependencies: ForumRouteDependencies,
) =>
  readActiveOrangeCheckByActorRef(
    db,
    actorRef,
    dependencies.entitlementsNonGateReads,
  ).pipe(
    Effect.flatMap(entitlement => {
      if (entitlement === null) {
        return Effect.succeed(notFound())
      }

      const recipientPubkey = url.searchParams.get('recipientPubkey') ?? ''
      const issuerPubkey = url.searchParams.get('issuerPubkey') ?? ''
      const relayUrls = url.searchParams.getAll('relay')

      return Effect.promise(async () => {
        try {
          const exported = await buildOrangeCheckNostrExport({
            entitlement,
            issuerPubkey,
            nowIso: dependencies.nowIso?.() ?? currentIsoTimestamp(),
            recipientPubkey,
            relayUrls,
          })

          return noStoreJsonResponse({ nostrExport: exported })
        } catch (error) {
          return error instanceof OrangeCheckNostrExportError
            ? badRequest(error.message)
            : serverError()
        }
      })
    }),
  )

const directTipMdkWebhookResponse = (
  request: Request,
  db: D1Database,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const body = yield* Effect.tryPromise({
      catch: error =>
        new ForumPaidActionError({
          kind: 'payment_verification_failed',
          reason:
            error instanceof Error
              ? error.message
              : 'Forum MDK webhook body could not be read.',
        }),
      try: () => request.text(),
    })
    const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()
    const verification = yield* Effect.tryPromise({
      catch: error =>
        new ForumPaidActionError({
          kind: 'payment_verification_failed',
          reason:
            error instanceof Error
              ? error.message
              : 'Forum MDK webhook could not be verified.',
        }),
      try: () =>
        verifyOpenAgentsForumMdkWebhook({
          body,
          config: dependencies.mdkWebhookConfig,
          headers: request.headers,
          nowIso,
        }),
    })

    if (verification._tag === 'Invalid') {
      return noStoreJsonResponse(
        {
          error: `mdk_webhook_${verification.reason}`,
          message: 'The Forum MDK webhook could not be verified.',
        },
        {
          status:
            verification.reason === 'missing_configuration'
              ? 503
              : verification.reason === 'invalid_signature'
                ? 401
                : 400,
        },
      )
    }

    const result = yield* reconcileForumDirectTipWebhook(
      dependencies.treasuryDb ?? db,
      {
      amount: verification.event.amount,
      attemptId: verification.event.attemptId,
      eventBodyDigestRef: verification.event.eventBodyDigestRef,
      paymentEvidence: {
        externalRef: verification.event.externalRef,
        paymentMode: verification.event.paymentMode,
        providerRef: verification.event.providerRef,
        redactedEvidenceRef: verification.event.redactedEvidenceRef,
        status: verification.event.status,
      },
      providerEventRef: verification.event.providerEventRef,
      signatureBindingRef: verification.event.signatureBindingRef,
    })

    return noStoreJsonResponse(result, {
      status: result.idempotent ? 200 : 201,
    })
  }).pipe(
    Effect.catch(error => Effect.succeed(paidActionFailureResponse(error))),
  )

const receiptLookupResponse = (db: D1Database, receiptRef: string) =>
  lookupForumPaidActionReceipt(db, receiptRef).pipe(
    Effect.map(receipt =>
      receipt === null
        ? notFound()
        : noStoreJsonResponse({
            ...receipt,
            generatedAt: currentIsoTimestamp(),
            staleness: forumReceiptStaleness,
          }),
    ),
    Effect.catch(error => Effect.succeed(paidActionFailureResponse(error))),
  )

const claimTipSettlementResponse = (
  request: Request,
  db: D1Database,
  receiptRef: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const actor = yield* agentForRequest(request, dependencies)
    const body = yield* decodeJsonBody(
      request,
      S.decodeUnknownSync(ForumTipSettlementClaimBody),
    )
    const result = yield* claimForumTipSettlement(
      dependencies.treasuryDb ?? db,
      {
      actorRef: actorRefForForumActor(actor),
      idempotencyKey,
      receiptRef,
      settlementEvidenceRefs: body.settlementEvidenceRefs,
      settlementRef: body.settlementRef,
      sourceRef: body.sourceRef,
    })

    return noStoreJsonResponse(result, {
      status: result.idempotent ? 200 : 201,
    })
  }).pipe(
    Effect.catch(error => Effect.succeed(paidActionFailureResponse(error))),
  )

const creatorEarningsResponse = (
  db: D1Database,
  actorRef: string,
  limit: number,
  dependencies: ForumRouteDependencies,
) =>
  readForumCreatorEarnings(
    db,
    { actorRef, limit },
    { nowIso: dependencies.nowIso ?? currentIsoTimestamp },
  ).pipe(
    Effect.map(noStoreJsonResponse),
    Effect.catch(error => Effect.succeed(writeFailureResponse(error))),
  )

const tipLeaderboardsResponse = (
  db: D1Database,
  limit: number,
  dependencies: ForumRouteDependencies,
) =>
  readForumTipLeaderboards(
    db,
    { limit },
    { nowIso: dependencies.nowIso ?? currentIsoTimestamp },
  ).pipe(
    Effect.map(noStoreJsonResponse),
    Effect.catch(error => Effect.succeed(writeFailureResponse(error))),
  )

const tipReconciliationResponse = (
  request: Request,
  db: D1Database,
  limit: number,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    yield* moderatorForRequest(request, dependencies)

    const url = new URL(request.url)
    const actorRef = url.searchParams.get('actorRef')?.trim() || null

    return yield* readForumTipReconciliation(
      db,
      { actorRef, limit },
      { nowIso: dependencies.nowIso ?? currentIsoTimestamp },
    ).pipe(Effect.map(noStoreJsonResponse))
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

// The public agent profile composes registration, approved owner
// claims, verified X-proof challenges, and orange-check entitlement
// live at read, and declares so (epic #4751 instances 1-2, #4744).
const agentProfileProjectionStaleness = liveAtReadStaleness([
  'agent_owner_claim_approved',
  'agent_owner_x_claim_verified',
  'agent_registration_updated',
  'orange_check_entitlement_changed',
])

const agentProfileResponse = (
  db: D1Database,
  profileRef: string,
  dependencies: ForumRouteDependencies = {},
) =>
  readForumAgentPublicProfile(db, profileRef).pipe(
    Effect.flatMap(profile =>
      profile === null
        ? Effect.succeed(notFound())
        : readActiveOrangeCheckByActorRef(
            db,
            profile.actor.actorRef,
            dependencies.entitlementsNonGateReads,
          ).pipe(
            Effect.map(entitlement =>
              noStoreJsonResponse({
                generatedAt: currentIsoTimestamp(),
                orangeCheck: orangeCheckBadgeProjection(entitlement),
                profile,
                staleness: agentProfileProjectionStaleness,
              }),
            ),
          ),
    ),
    Effect.catch(error => Effect.succeed(writeFailureResponse(error))),
  )

const profileTipSummary = (db: D1Database, actorRef: string) =>
  Effect.all([
    readForumTipRecipientReadinessForActor(db, actorRef).pipe(
      Effect.map(readiness => readiness.tippingAvailable),
      Effect.catch(() => Effect.succeed(false)),
    ),
    readForumCreatorEarnings(
      db,
      { actorRef },
      { nowIso: currentIsoTimestamp },
    ).pipe(
      Effect.map(earnings => ({
        settledCount: earnings.summary.settledCount,
        totalSettledSats: earnings.summary.totalSettledSats,
      })),
      Effect.catch(() =>
        Effect.succeed({ settledCount: 0, totalSettledSats: 0 }),
      ),
    ),
  ]).pipe(
    Effect.map(([tippingAvailable, summary]) => ({
      settledCount: summary.settledCount,
      tippingAvailable,
      totalSettledSats: summary.totalSettledSats,
    })),
  )

const agentProfilePageResponse = (
  db: D1Database,
  profileRef: string,
  dependencies: ForumRouteDependencies = {},
) =>
  readForumAgentPublicProfile(db, profileRef).pipe(
    Effect.flatMap(profile =>
      profile === null
        ? Effect.succeed(notFound())
        : Effect.all([
            readActiveOrangeCheckByActorRef(
              db,
              profile.actor.actorRef,
              dependencies.entitlementsNonGateReads,
            ),
            profileTipSummary(db, profile.actor.actorRef),
          ]).pipe(
            Effect.map(([entitlement, tips]) =>
              htmlResponse(
                renderAgentProfilePage(
                  profile,
                  entitlement !== null && entitlement.state === 'active',
                  tips,
                ),
              ),
            ),
          ),
    ),
    Effect.catch(error => Effect.succeed(writeFailureResponse(error))),
  )

const agentProfileRedirectResponse = (db: D1Database, profileRef: string) =>
  readForumAgentPublicProfile(db, profileRef).pipe(
    Effect.map(profile =>
      profile === null ? notFound() : redirectResponse(profile.publicUrl),
    ),
    Effect.catch(error => Effect.succeed(writeFailureResponse(error))),
  )

const watchForumResponse = (
  request: Request,
  db: D1Database,
  input: Readonly<{
    forumId: string
    topicId: string | null
    watchKind: 'forum' | 'topic'
  }>,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const existing = yield* readForumWatchByIdempotencyKey(db, idempotencyKey)
    const actor = yield* actorForRequest(request, dependencies)
    const actorRef = actorRefForForumActor(actor)
    const nowEpochMillis = dependencies.nowEpochMillis ?? currentEpochMillis
    const writer = yield* buildForumWriterContext({
      actor,
      grant: forumParticipationGrantForActor(
        actor,
        input.forumId,
        nowEpochMillis,
      ),
      nowEpochMillis,
      paymentProofRef: null,
      requiredScope: 'forum.watch',
      targetForumId: input.forumId,
      targetOwnerUserId: null,
      targetTeamId: null,
    })

    if (existing !== null) {
      return noStoreJsonResponse(
        decodeParticipationWriteResponse({
          action: 'watch',
          actorRef: writer.actor.actorRef,
          id: existing.id,
          idempotencyKey: existing.idempotency_key,
          idempotent: true,
          target: {
            actorRef: null,
            forumId: existing.forum_id,
            postId: null,
            topicId: existing.topic_id,
          },
        }),
      )
    }

    const id = yield* watchForumTarget(db, {
      actorRef,
      forumId: input.forumId,
      idempotencyKey,
      topicId: input.topicId,
      watchKind: input.watchKind,
    })

    return noStoreJsonResponse(
      decodeParticipationWriteResponse({
        action: 'watch',
        actorRef: writer.actor.actorRef,
        id,
        idempotencyKey,
        idempotent: false,
        target: {
          actorRef: null,
          forumId: input.forumId,
          postId: null,
          topicId: input.topicId,
        },
      }),
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const bookmarkForumResponse = (
  request: Request,
  db: D1Database,
  input: Readonly<{
    bookmarkKind: 'post' | 'topic'
    forumId: string
    postId: string | null
    topicId: string
  }>,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const existing = yield* readForumBookmarkByIdempotencyKey(
      db,
      idempotencyKey,
    )
    const actor = yield* actorForRequest(request, dependencies)
    const actorRef = actorRefForForumActor(actor)
    const nowEpochMillis = dependencies.nowEpochMillis ?? currentEpochMillis
    const writer = yield* buildForumWriterContext({
      actor,
      grant: forumParticipationGrantForActor(
        actor,
        input.forumId,
        nowEpochMillis,
      ),
      nowEpochMillis,
      paymentProofRef: null,
      requiredScope: 'forum.bookmark',
      targetForumId: input.forumId,
      targetOwnerUserId: null,
      targetTeamId: null,
    })

    if (existing !== null) {
      return noStoreJsonResponse(
        decodeParticipationWriteResponse({
          action: 'bookmark',
          actorRef: writer.actor.actorRef,
          id: existing.id,
          idempotencyKey: existing.idempotency_key,
          idempotent: true,
          target: {
            actorRef: null,
            forumId: input.forumId,
            postId: existing.post_id,
            topicId: existing.topic_id,
          },
        }),
      )
    }

    const id = yield* bookmarkForumTarget(db, {
      actorRef,
      bookmarkKind: input.bookmarkKind,
      idempotencyKey,
      postId: input.postId,
      topicId: input.topicId,
    })

    return noStoreJsonResponse(
      decodeParticipationWriteResponse({
        action: 'bookmark',
        actorRef: writer.actor.actorRef,
        id,
        idempotencyKey,
        idempotent: false,
        target: {
          actorRef: null,
          forumId: input.forumId,
          postId: input.postId,
          topicId: input.topicId,
        },
      }),
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const followActorResponse = (
  request: Request,
  db: D1Database,
  targetActorRef: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const targetProfile = yield* readForumAgentPublicProfile(db, targetActorRef)

    if (targetProfile === null) {
      return notFound()
    }

    const actor = yield* actorForRequest(request, dependencies)
    const actorRef = actorRefForForumActor(actor)

    if (actorRef === targetProfile.actor.actorRef) {
      return badRequest('agents cannot follow themselves')
    }

    const existing = yield* readForumFollowByIdempotencyKey(db, idempotencyKey)

    if (existing !== null) {
      return noStoreJsonResponse(
        decodeParticipationWriteResponse({
          action: 'follow',
          actorRef,
          id: existing.id,
          idempotencyKey: existing.idempotency_key,
          idempotent: true,
          target: {
            actorRef: existing.target_actor_ref,
            forumId: null,
            postId: null,
            topicId: null,
          },
        }),
      )
    }

    const id = yield* followForumActor(db, {
      actorRef,
      idempotencyKey,
      targetActorRef: targetProfile.actor.actorRef,
    })

    return noStoreJsonResponse(
      decodeParticipationWriteResponse({
        action: 'follow',
        actorRef,
        id,
        idempotencyKey,
        idempotent: false,
        target: {
          actorRef: targetProfile.actor.actorRef,
          forumId: null,
          postId: null,
          topicId: null,
        },
      }),
      { status: 201 },
    )
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const agentNotificationsResponse = (
  request: Request,
  db: D1Database,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const actor = yield* actorForRequest(request, dependencies)
    const actorRef = actorRefForForumActor(actor)
    const actorSlug = actorSlugForForumActor(actor)
    const url = new URL(request.url)
    const limitValue = Number(url.searchParams.get('limit') ?? '50')
    const limit = Number.isFinite(limitValue) ? limitValue : 50
    const unreadOnly = url.searchParams.get('unread') === 'true'
    const generatedAt = dependencies.nowIso?.() ?? currentIsoTimestamp()
    const notifications = yield* readForumAgentNotifications(db, {
      actorRef,
      actorSlug,
      generatedAt,
      limit,
    })

    // When `?unread=true` is set, return only unread notifications in the
    // array. `summary` is intentionally left untouched: `summary.unreadCount`
    // stays the true server-computed unread count, and `summary.mentionCount`
    // (and the other per-kind counts) remain TOTAL counts across all
    // notifications regardless of read state.
    const filteredNotifications = unreadOnly
      ? {
          ...notifications,
          notifications: notifications.notifications.filter(
            notification => notification.readState === 'unread',
          ),
        }
      : notifications

    return noStoreJsonResponse(filteredNotifications)
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

const notificationReadResponseBody = (
  input: Readonly<{
    actorRef: string
    id: string
    idempotencyKey: string
    idempotent: boolean
    notificationId: string
    readAt: string
  }>,
) => ({
  actorRef: input.actorRef,
  id: input.id,
  idempotencyKey: input.idempotencyKey,
  idempotent: input.idempotent,
  notificationId: input.notificationId,
  readAt: input.readAt,
})

const markAgentNotificationReadResponse = (
  request: Request,
  db: D1Database,
  notificationId: string,
  dependencies: ForumRouteDependencies,
) =>
  Effect.gen(function* () {
    const idempotencyKey = idempotencyKeyFromRequest(request)

    if (idempotencyKey === undefined) {
      return badRequest('Idempotency-Key header is required')
    }

    const actor = yield* actorForRequest(request, dependencies)
    const actorRef = actorRefForForumActor(actor)
    const existingByKey = yield* readForumNotificationReadByIdempotencyKey(db, {
      actorRef,
      idempotencyKey,
    })

    if (existingByKey !== null) {
      return existingByKey.notification_id === notificationId
        ? noStoreJsonResponse(
            notificationReadResponseBody({
              actorRef: existingByKey.actor_ref,
              id: existingByKey.id,
              idempotencyKey: existingByKey.idempotency_key,
              idempotent: true,
              notificationId: existingByKey.notification_id,
              readAt: existingByKey.read_at,
            }),
          )
        : idempotencyConflictResponse()
    }

    const existingByNotification =
      yield* readForumNotificationReadByNotificationId(db, {
        actorRef,
        notificationId,
      })

    if (existingByNotification !== null) {
      return noStoreJsonResponse(
        notificationReadResponseBody({
          actorRef: existingByNotification.actor_ref,
          id: existingByNotification.id,
          idempotencyKey: existingByNotification.idempotency_key,
          idempotent: true,
          notificationId: existingByNotification.notification_id,
          readAt: existingByNotification.read_at,
        }),
      )
    }

    const makeId = dependencies.makeId ?? randomUuid
    const readAt = dependencies.nowIso?.() ?? currentIsoTimestamp()
    const recorded = yield* recordForumNotificationRead(db, {
      actorRef,
      id: makeId(),
      idempotencyKey,
      notificationId,
      readAt,
    })

    return noStoreJsonResponse(recorded, { status: 201 })
  }).pipe(Effect.catch(error => Effect.succeed(writeFailureResponse(error))))

export const makeForumRoutes = (dependencies: ForumRouteDependencies = {}) => ({
  routeForumRequest: (
    request: Request,
    db: D1Database,
    requestDependencies: ForumRouteDependencies = dependencies,
  ) => {
    const url = new URL(request.url)

    // Per-thread Open Graph / Twitter Card image. Renders the thread title onto
    // a 1200x630 branded SVG so a shared `/forum/t/{id}` link carries a visual.
    // `default` and an unknown/malformed topic both yield the branded default
    // image rather than an error, so a crawler never sees a broken thumbnail.
    const forumOgImageMatch = /^\/og\/forum\/([^/]+)\.svg$/.exec(url.pathname)

    if (forumOgImageMatch !== null) {
      if (request.method !== 'GET' && request.method !== 'HEAD') {
        return Effect.succeed(methodNotAllowed(['GET', 'HEAD']))
      }

      const rawSegment = forumOgImageMatch[1]
      const topicSegment = decodePathSegment(rawSegment)

      if (topicSegment === undefined || topicSegment === 'default') {
        return Effect.succeed(forumThreadOgImageResponse(null))
      }

      return readForumTopicById(db, topicSegment).pipe(
        Effect.map(topic => forumThreadOgImageResponse(topic?.title ?? null)),
        Effect.catch(() => Effect.succeed(forumThreadOgImageResponse(null))),
      )
    }

    const forumAgentProfilePageMatch = /^\/forum\/u\/([^/]+)\/([^/]+)$/.exec(
      url.pathname,
    )

    if (forumAgentProfilePageMatch !== null) {
      const actorId = decodePathSegment(forumAgentProfilePageMatch[1])

      if (actorId === undefined || actorId.trim().length === 0) {
        return Effect.succeed(badRequest('agent profile actor id is malformed'))
      }

      return request.method === 'GET'
        ? agentProfilePageResponse(db, actorId, requestDependencies)
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const registeredAgentProfileRedirectMatch = /^\/agents\/([^/]+)$/.exec(
      url.pathname,
    )

    if (registeredAgentProfileRedirectMatch !== null) {
      const profileRef = decodePathSegment(
        registeredAgentProfileRedirectMatch[1],
      )
      const reservedPublicAgentRefs = new Set(['adjutant', 'artanis'])

      if (
        profileRef === undefined ||
        profileRef.trim().length === 0 ||
        reservedPublicAgentRefs.has(profileRef)
      ) {
        return undefined
      }

      return request.method === 'GET'
        ? agentProfileRedirectResponse(db, profileRef)
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    if (url.pathname === '/api/forum') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      const shouldIncludeUnlisted = includeUnlisted(url)

      return (
        shouldIncludeUnlisted
          ? authorizeUnlistedDiscovery(request, requestDependencies)
          : Effect.void
      ).pipe(
        Effect.flatMap(() =>
          publicReadResponse(
            readForumBoardIndex(db, {
              includeUnlisted: shouldIncludeUnlisted,
            }),
          ),
        ),
        Effect.catchTag('ForumWriterAuthFailure', () =>
          Effect.succeed(unauthorized()),
        ),
      )
    }

    if (url.pathname === '/api/forum/work-requests') {
      if (request.method === 'GET') {
        return listForumWorkRequestsResponse(db, url)
      }

      if (request.method === 'POST') {
        return createForumWorkRequestResponse(request, db, requestDependencies)
      }

      return Effect.succeed(methodNotAllowed(['GET', 'POST']))
    }

    if (url.pathname === '/api/forum/work-requests/relay-events') {
      return request.method === 'POST'
        ? ingestRelayNativeForumWorkRequestResponse(
            request,
            db,
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const workRequestStatusMatch =
      /^\/api\/forum\/work-requests\/([^/]+)$/.exec(url.pathname)

    if (workRequestStatusMatch !== null) {
      const workRequestId = decodePathSegment(workRequestStatusMatch[1])

      if (workRequestId === undefined) {
        return Effect.succeed(badRequest('workRequestId is malformed'))
      }

      return request.method === 'GET'
        ? readForumWorkRequestStatusResponse(db, workRequestId)
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const workRequestRelayOffersMatch =
      /^\/api\/forum\/work-requests\/([^/]+)\/offers\/relay-events$/.exec(
        url.pathname,
      )

    if (workRequestRelayOffersMatch !== null) {
      const workRequestId = decodePathSegment(workRequestRelayOffersMatch[1])

      if (workRequestId === undefined) {
        return Effect.succeed(badRequest('workRequestId is malformed'))
      }

      return request.method === 'POST'
        ? ingestRelayNativeForumWorkRequestOfferResponse(
            request,
            db,
            workRequestId,
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const workRequestOffersMatch =
      /^\/api\/forum\/work-requests\/([^/]+)\/offers$/.exec(url.pathname)

    if (workRequestOffersMatch !== null) {
      const workRequestId = decodePathSegment(workRequestOffersMatch[1])

      if (workRequestId === undefined) {
        return Effect.succeed(badRequest('workRequestId is malformed'))
      }

      if (request.method === 'GET') {
        return listForumWorkRequestOffersResponse(db, workRequestId)
      }

      if (request.method === 'POST') {
        return submitForumWorkRequestOfferResponse(
          request,
          db,
          workRequestId,
          requestDependencies,
        )
      }

      return Effect.succeed(methodNotAllowed(['GET', 'POST']))
    }

    const workRequestResultsMatch =
      /^\/api\/forum\/work-requests\/([^/]+)\/results$/.exec(url.pathname)

    if (workRequestResultsMatch !== null) {
      const workRequestId = decodePathSegment(workRequestResultsMatch[1])

      if (workRequestId === undefined) {
        return Effect.succeed(badRequest('workRequestId is malformed'))
      }

      return request.method === 'POST'
        ? submitForumWorkRequestResultResponse(
            request,
            db,
            workRequestId,
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const workRequestReleaseMatch =
      /^\/api\/forum\/work-requests\/([^/]+)\/release$/.exec(url.pathname)

    if (workRequestReleaseMatch !== null) {
      const workRequestId = decodePathSegment(workRequestReleaseMatch[1])

      if (workRequestId === undefined) {
        return Effect.succeed(badRequest('workRequestId is malformed'))
      }

      return request.method === 'POST'
        ? releaseForumWorkRequestEscrowResponse(
            request,
            db,
            workRequestId,
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const workRequestAcceptanceMatch =
      /^\/api\/forum\/work-requests\/([^/]+)\/acceptances$/.exec(url.pathname)

    if (workRequestAcceptanceMatch !== null) {
      const workRequestId = decodePathSegment(workRequestAcceptanceMatch[1])

      if (workRequestId === undefined) {
        return Effect.succeed(badRequest('workRequestId is malformed'))
      }

      return request.method === 'POST'
        ? acceptForumWorkRequestOfferResponse(
            request,
            db,
            workRequestId,
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const workRequestLifecycleMatch =
      /^\/api\/forum\/work-requests\/([^/]+)\/lifecycle-posts$/.exec(
        url.pathname,
      )

    if (workRequestLifecycleMatch !== null) {
      const workRequestId = decodePathSegment(workRequestLifecycleMatch[1])

      if (workRequestId === undefined) {
        return Effect.succeed(badRequest('workRequestId is malformed'))
      }

      return request.method === 'POST'
        ? createForumWorkRequestLifecycleResponse(
            request,
            db,
            workRequestId,
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    if (url.pathname === '/api/forum/search') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      const query = url.searchParams.get('q')?.trim() ?? ''

      if (query.length < 2 || query.length > 120) {
        return Effect.succeed(
          badRequest('q must be between 2 and 120 characters'),
        )
      }

      const shouldIncludeUnlisted = includeUnlisted(url)

      return (
        shouldIncludeUnlisted
          ? authorizeUnlistedDiscovery(request, requestDependencies)
          : Effect.void
      ).pipe(
        Effect.flatMap(() =>
          publicSearchResponse(
            searchForumPublicContent(db, {
              includeUnlisted: shouldIncludeUnlisted,
              query,
            }),
          ),
        ),
        Effect.catchTag('ForumWriterAuthFailure', () =>
          Effect.succeed(unauthorized()),
        ),
      )
    }

    if (url.pathname === '/api/forum/launch-status') {
      return request.method === 'GET'
        ? countActiveOrangeChecks(
            db,
            requestDependencies.entitlementsNonGateReads,
          ).pipe(
            Effect.map(orangeChecksSold =>
              noStoreJsonResponse({
                ...forumLaunchGateStatus(),
                generatedAt: currentIsoTimestamp(),
                orangeChecksSold,
                staleness: forumLaunchStatusStaleness,
              }),
            ),
          )
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    if (url.pathname === '/api/forum/tip-leaderboards') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      const limit = forumListLimitFromUrl(url)

      if (limit instanceof Response) {
        return Effect.succeed(limit)
      }

      return tipLeaderboardsResponse(db, limit, requestDependencies)
    }

    if (url.pathname === '/api/forum/moderation/tip-earnings') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      const limit = forumListLimitFromUrl(url)

      if (limit instanceof Response) {
        return Effect.succeed(limit)
      }

      return tipReconciliationResponse(request, db, limit, requestDependencies)
    }

    if (url.pathname === '/api/forum/moderation/queue') {
      return request.method === 'GET'
        ? moderationQueueResponse(request, db, requestDependencies)
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const moderationReportMatch =
      /^\/api\/forum\/moderation\/reports\/([^/]+)$/.exec(url.pathname)

    if (moderationReportMatch !== null) {
      const reportId = decodePathSegment(moderationReportMatch[1])

      if (reportId === undefined) {
        return Effect.succeed(badRequest('reportId is malformed'))
      }

      return request.method === 'GET'
        ? moderationItemResponse(
            request,
            db,
            { itemId: reportId, itemKind: 'report' },
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const moderationReportActionMatch =
      /^\/api\/forum\/moderation\/reports\/([^/]+)\/(mark-reviewed|dismiss)$/.exec(
        url.pathname,
      )

    if (moderationReportActionMatch !== null) {
      const reportId = decodePathSegment(moderationReportActionMatch[1])
      const actionSlug = decodePathSegment(moderationReportActionMatch[2])

      if (reportId === undefined || actionSlug === undefined) {
        return Effect.succeed(badRequest('report moderation path is malformed'))
      }

      const status = actionSlug === 'dismiss' ? 'dismissed' : 'resolved'

      return request.method === 'POST'
        ? moderationActionResponse(
            request,
            db,
            {
              actionKind: `moderator_${actionSlug.replace('-', '_')}_report`,
              reportId,
              targetId: reportId,
              targetKind: 'report',
              update: () => updateForumReportStatus(db, { reportId, status }),
            },
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const moderationPostMatch =
      /^\/api\/forum\/moderation\/posts\/([^/]+)$/.exec(url.pathname)

    if (moderationPostMatch !== null) {
      const postId = decodePathSegment(moderationPostMatch[1])

      if (postId === undefined) {
        return Effect.succeed(badRequest('postId is malformed'))
      }

      return request.method === 'GET'
        ? moderationItemResponse(
            request,
            db,
            { itemId: postId, itemKind: 'post_review' },
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const moderationPostActionMatch =
      /^\/api\/forum\/moderation\/posts\/([^/]+)\/(approve|hide)$/.exec(
        url.pathname,
      )

    if (moderationPostActionMatch !== null) {
      const postId = decodePathSegment(moderationPostActionMatch[1])
      const actionSlug = decodePathSegment(moderationPostActionMatch[2])

      if (postId === undefined || actionSlug === undefined) {
        return Effect.succeed(badRequest('post moderation path is malformed'))
      }

      const state = actionSlug === 'hide' ? 'hidden' : 'visible'

      return request.method === 'POST'
        ? moderationActionResponse(
            request,
            db,
            {
              actionKind: `moderator_${actionSlug}_post`,
              targetId: postId,
              targetKind: 'post',
              update: () =>
                updateForumPostModerationState(db, { postId, state }),
            },
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const moderationTopicMatch =
      /^\/api\/forum\/moderation\/topics\/([^/]+)$/.exec(url.pathname)

    if (moderationTopicMatch !== null) {
      const topicId = decodePathSegment(moderationTopicMatch[1])

      if (topicId === undefined) {
        return Effect.succeed(badRequest('topicId is malformed'))
      }

      return request.method === 'GET'
        ? moderationItemResponse(
            request,
            db,
            { itemId: topicId, itemKind: 'topic_review' },
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const moderationTopicActionMatch =
      /^\/api\/forum\/moderation\/topics\/([^/]+)\/(lock|unlock|archive|hide|pin|unpin)$/.exec(
        url.pathname,
      )

    if (moderationTopicActionMatch !== null) {
      const topicId = decodePathSegment(moderationTopicActionMatch[1])
      const actionSlug = decodePathSegment(moderationTopicActionMatch[2])

      if (topicId === undefined || actionSlug === undefined) {
        return Effect.succeed(badRequest('topic moderation path is malformed'))
      }

      if (actionSlug === 'pin' || actionSlug === 'unpin') {
        const pinState = actionSlug === 'pin' ? 'sticky' : 'normal'

        return request.method === 'POST'
          ? moderationActionResponse(
              request,
              db,
              {
                actionKind: `moderator_${actionSlug}_topic`,
                targetId: topicId,
                targetKind: 'topic',
                update: () =>
                  updateForumTopicPinState(db, { pinState, topicId }),
              },
              requestDependencies,
            )
          : Effect.succeed(methodNotAllowed(['POST']))
      }

      const state =
        actionSlug === 'unlock'
          ? 'open'
          : actionSlug === 'archive'
            ? 'archived'
            : actionSlug === 'hide'
              ? 'hidden'
              : 'locked'

      return request.method === 'POST'
        ? moderationActionResponse(
            request,
            db,
            {
              actionKind: `moderator_${actionSlug}_topic`,
              targetId: topicId,
              targetKind: 'topic',
              update: () =>
                updateForumTopicModerationState(db, { state, topicId }),
            },
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const contextActivityMatch =
      /^\/api\/forum\/contexts\/(site|workroom)\/([^/]+)\/activity$/.exec(
        url.pathname,
      )

    if (contextActivityMatch !== null) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      const contextKind = contextActivityMatch[1] as 'site' | 'workroom'
      const contextId = decodePathSegment(contextActivityMatch[2])

      if (contextId === undefined || contextId.trim().length === 0) {
        return Effect.succeed(badRequest('context id is malformed'))
      }

      const limit = forumListLimitFromUrl(url)

      if (limit instanceof Response) {
        return Effect.succeed(limit)
      }

      return publicListResponse(
        readForumContextActivity(db, {
          contextId,
          contextKind,
          limit,
        }),
      )
    }

    if (url.pathname === '/api/forum/posts') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      const limit = forumListLimitFromUrl(url)

      if (limit instanceof Response) {
        return Effect.succeed(limit)
      }

      const cursorRef = url.searchParams.get('cursor')?.trim() ?? null
      const cursor =
        cursorRef === null || cursorRef.length === 0
          ? null
          : decodeForumPostListCursor(cursorRef)

      if (cursorRef !== null && cursor === null) {
        return Effect.succeed(badRequest('cursor is malformed'))
      }

      const shouldIncludeUnlisted = includeUnlisted(url)
      const forumRef =
        url.searchParams.get('forumRef')?.trim() ??
        url.searchParams.get('forumId')?.trim() ??
        null
      const topicId = url.searchParams.get('topicId')?.trim() ?? null

      return (
        shouldIncludeUnlisted
          ? authorizeUnlistedDiscovery(request, requestDependencies)
          : Effect.void
      ).pipe(
        Effect.flatMap(() =>
          publicListResponse(
            readForumPostList(db, {
              cursor,
              cursorRef,
              forumRef,
              includeUnlisted: shouldIncludeUnlisted,
              limit,
              topicId,
            }),
          ),
        ),
        Effect.catchTag('ForumWriterAuthFailure', () =>
          Effect.succeed(unauthorized()),
        ),
      )
    }

    if (url.pathname === '/api/agents/notifications') {
      return request.method === 'GET'
        ? agentNotificationsResponse(request, db, requestDependencies)
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const agentNotificationReadMatch =
      /^\/api\/agents\/notifications\/([^/]+)\/read$/.exec(url.pathname)

    if (agentNotificationReadMatch !== null) {
      const notificationId = decodePathSegment(agentNotificationReadMatch[1])

      if (notificationId === undefined || notificationId.trim().length === 0) {
        return Effect.succeed(badRequest('notification id is malformed'))
      }

      return request.method === 'POST'
        ? markAgentNotificationReadResponse(
            request,
            db,
            notificationId,
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const agentProfileMatch = /^\/api\/agents\/profiles\/([^/]+)$/.exec(
      url.pathname,
    )

    if (agentProfileMatch !== null) {
      const profileRef = decodePathSegment(agentProfileMatch[1])

      if (profileRef === undefined) {
        return Effect.succeed(badRequest('agent profile ref is malformed'))
      }

      return request.method === 'GET'
        ? agentProfileResponse(db, profileRef, requestDependencies)
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    if (url.pathname === '/api/forum/paid-actions/preview') {
      return request.method === 'POST'
        ? previewPaidActionResponse(request, db, requestDependencies)
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    if (url.pathname === '/api/forum/tip-recipient-wallets/admissions') {
      return request.method === 'POST'
        ? tipRecipientAdmissionResponse(request, db, requestDependencies)
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    if (url.pathname === '/api/forum/tip-recipient-wallets/claims') {
      return request.method === 'POST'
        ? tipRecipientWalletClaimResponse(request, db, requestDependencies)
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    if (url.pathname === '/api/forum/paid-actions/private-payment') {
      return request.method === 'POST'
        ? privatePaidActionPaymentResponse(request, db, requestDependencies)
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    if (url.pathname === '/api/forum/paid-actions/redeem') {
      return request.method === 'POST'
        ? redeemPaidActionResponse(request, db, requestDependencies)
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const receiptSettlementClaimMatch =
      /^\/api\/forum\/receipts\/([^/]+)\/settlement-claims$/.exec(url.pathname)

    if (receiptSettlementClaimMatch !== null) {
      const receiptRef = decodePathSegment(receiptSettlementClaimMatch[1])

      if (receiptRef === undefined) {
        return Effect.succeed(badRequest('receiptId is malformed'))
      }

      return request.method === 'POST'
        ? claimTipSettlementResponse(
            request,
            db,
            receiptRef,
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const receiptMatch = /^\/api\/forum\/receipts\/([^/]+)$/.exec(url.pathname)

    if (receiptMatch !== null) {
      const receiptRef = decodePathSegment(receiptMatch[1])

      if (receiptRef === undefined) {
        return Effect.succeed(badRequest('receiptId is malformed'))
      }

      return request.method === 'GET'
        ? receiptLookupResponse(db, receiptRef)
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const directTipMatch = /^\/api\/forum\/direct-tips\/([^/]+)$/.exec(
      url.pathname,
    )

    if (directTipMatch !== null) {
      const attemptId = decodePathSegment(directTipMatch[1])

      if (attemptId === undefined) {
        return Effect.succeed(badRequest('direct tip id is malformed'))
      }

      return request.method === 'GET'
        ? directTipStatusResponse(db, attemptId)
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const pylonTipLadderMatch =
      /^\/api\/pylons\/([^/]+)\/tips\/ladder$/.exec(url.pathname)

    if (pylonTipLadderMatch !== null) {
      const pylonRef = decodePathSegment(pylonTipLadderMatch[1])

      if (pylonRef === undefined) {
        return Effect.succeed(badRequest('pylon tip ladder path is malformed'))
      }

      return request.method === 'POST'
        ? pylonTipLadderResponse(request, db, pylonRef, requestDependencies)
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const postTipLadderMatch =
      /^\/api\/forum\/posts\/([^/]+)\/tips\/ladder$/.exec(url.pathname)

    if (postTipLadderMatch !== null) {
      const postId = decodePathSegment(postTipLadderMatch[1])

      if (postId === undefined) {
        return Effect.succeed(badRequest('tip ladder path is malformed'))
      }

      return request.method === 'POST'
        ? tipLadderResponse(request, db, postId, requestDependencies)
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const postDirectTipMatch =
      /^\/api\/forum\/posts\/([^/]+)\/direct-tips$/.exec(url.pathname)

    if (postDirectTipMatch !== null) {
      const postId = decodePathSegment(postDirectTipMatch[1])

      if (postId === undefined) {
        return Effect.succeed(badRequest('post direct tip path is malformed'))
      }

      return request.method === 'POST'
        ? submitDirectTipResponse(request, db, postId, requestDependencies)
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    if (url.pathname === '/api/forum/paid-actions/mdk/webhooks') {
      return request.method === 'POST'
        ? directTipMdkWebhookResponse(request, db, requestDependencies)
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const postPaidActionMatch =
      /^\/api\/forum\/posts\/([^/]+)\/(rewards|boosts|endorsements|down-signals)$/.exec(
        url.pathname,
      )

    if (postPaidActionMatch !== null) {
      const postId = decodePathSegment(postPaidActionMatch[1])
      const actionSlug = decodePathSegment(postPaidActionMatch[2])

      if (postId === undefined || actionSlug === undefined) {
        return Effect.succeed(badRequest('post paid action path is malformed'))
      }

      const actionKind =
        actionSlug === 'down-signals'
          ? 'post_down_signal'
          : actionSlug === 'rewards'
            ? 'post_reward'
            : 'post_boost'

      return request.method === 'POST'
        ? previewAliasPaidActionResponse(
            request,
            db,
            {
              actionKind,
              routeParams: { postId },
              target: S.decodeUnknownSync(ForumPaidActionTarget)({
                forumId: null,
                postId,
                topicId: null,
              }),
            },
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const topicPaidActionMatch =
      /^\/api\/forum\/topics\/([^/]+)\/(boosts|funds)$/.exec(url.pathname)

    if (topicPaidActionMatch !== null) {
      const topicId = decodePathSegment(topicPaidActionMatch[1])
      const actionSlug = decodePathSegment(topicPaidActionMatch[2])

      if (topicId === undefined || actionSlug === undefined) {
        return Effect.succeed(badRequest('topic paid action path is malformed'))
      }

      const actionKind = actionSlug === 'funds' ? 'topic_fund' : 'topic_boost'

      return request.method === 'POST'
        ? previewAliasPaidActionResponse(
            request,
            db,
            {
              actionKind,
              routeParams: { topicId },
              target: S.decodeUnknownSync(ForumPaidActionTarget)({
                forumId: null,
                postId: null,
                topicId,
              }),
            },
            requestDependencies,
          )
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const actorFollowMatch = /^\/api\/forum\/actors\/([^/]+)\/follows$/.exec(
      url.pathname,
    )

    if (actorFollowMatch !== null) {
      const actorRef = decodePathSegment(actorFollowMatch[1])

      if (actorRef === undefined) {
        return Effect.succeed(badRequest('actor ref is malformed'))
      }

      return request.method === 'POST'
        ? followActorResponse(request, db, actorRef, requestDependencies)
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const actorTipEarningsMatch =
      /^\/api\/forum\/actors\/([^/]+)\/tip-earnings$/.exec(url.pathname)

    if (actorTipEarningsMatch !== null) {
      const actorRef = decodePathSegment(actorTipEarningsMatch[1])

      if (actorRef === undefined) {
        return Effect.succeed(badRequest('actor ref is malformed'))
      }

      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      const limit = forumListLimitFromUrl(url)

      if (limit instanceof Response) {
        return Effect.succeed(limit)
      }

      return creatorEarningsResponse(db, actorRef, limit, requestDependencies)
    }

    const actorOrangeCheckNostrExportMatch =
      /^\/api\/forum\/actors\/([^/]+)\/orange-check\/nostr-export$/.exec(
        url.pathname,
      )

    if (actorOrangeCheckNostrExportMatch !== null) {
      const actorRef = decodePathSegment(actorOrangeCheckNostrExportMatch[1])

      if (actorRef === undefined) {
        return Effect.succeed(badRequest('actor ref is malformed'))
      }

      return request.method === 'GET'
        ? orangeCheckNostrExportResponse(db, actorRef, url, requestDependencies)
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const actorProfileMatch = /^\/api\/forum\/actors\/([^/]+)\/profile$/.exec(
      url.pathname,
    )

    if (actorProfileMatch !== null) {
      const actorRef = decodePathSegment(actorProfileMatch[1])

      if (actorRef === undefined) {
        return Effect.succeed(badRequest('actor ref is malformed'))
      }

      return request.method === 'GET'
        ? agentProfileResponse(db, actorRef, requestDependencies)
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const forumWatchMatch = /^\/api\/forum\/forums\/([^/]+)\/watches$/.exec(
      url.pathname,
    )

    if (forumWatchMatch !== null) {
      const forumRef = decodePathSegment(forumWatchMatch[1])

      if (forumRef === undefined) {
        return Effect.succeed(badRequest('forumId is malformed'))
      }

      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return readForumSummaryByRef(db, forumRef, { allowUnlisted: true }).pipe(
        Effect.flatMap(forum =>
          forum === null
            ? Effect.succeed(notFound())
            : watchForumResponse(
                request,
                db,
                {
                  forumId: forum.forumId,
                  topicId: null,
                  watchKind: 'forum',
                },
                requestDependencies,
              ),
        ),
        Effect.catch(error => Effect.succeed(writeFailureResponse(error))),
      )
    }

    const forumTopicsMatch = /^\/api\/forum\/forums\/([^/]+)\/topics$/.exec(
      url.pathname,
    )

    if (forumTopicsMatch !== null) {
      const forumRef = decodePathSegment(forumTopicsMatch[1])

      if (forumRef === undefined) {
        return Effect.succeed(badRequest('forumId is malformed'))
      }

      if (request.method === 'GET') {
        return publicReadResponse(readForumTopicList(db, forumRef))
      }

      if (request.method === 'POST') {
        return createTopicResponse(request, db, forumRef, requestDependencies)
      }

      return Effect.succeed(methodNotAllowed(['GET', 'POST']))
    }

    const forumMatch = /^\/api\/forum\/forums\/([^/]+)$/.exec(url.pathname)

    if (forumMatch !== null) {
      const forumRef = decodePathSegment(forumMatch[1])

      if (forumRef === undefined) {
        return Effect.succeed(badRequest('forumId is malformed'))
      }

      return request.method === 'GET'
        ? publicReadResponse(
            readForumSummaryByRef(db, forumRef, { allowUnlisted: true }),
          )
        : Effect.succeed(methodNotAllowed(['GET']))
    }

    const topicWatchMatch = /^\/api\/forum\/topics\/([^/]+)\/watches$/.exec(
      url.pathname,
    )

    if (topicWatchMatch !== null) {
      const topicId = decodePathSegment(topicWatchMatch[1])

      if (topicId === undefined) {
        return Effect.succeed(badRequest('topicId is malformed'))
      }

      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return readForumTopicDetail(db, topicId).pipe(
        Effect.flatMap(detail =>
          detail === null
            ? Effect.succeed(notFound())
            : watchForumResponse(
                request,
                db,
                {
                  forumId: detail.topic.forumId,
                  topicId: detail.topic.topicId,
                  watchKind: 'topic',
                },
                requestDependencies,
              ),
        ),
        Effect.catch(error => Effect.succeed(writeFailureResponse(error))),
      )
    }

    const topicBookmarkMatch =
      /^\/api\/forum\/topics\/([^/]+)\/bookmarks$/.exec(url.pathname)

    if (topicBookmarkMatch !== null) {
      const topicId = decodePathSegment(topicBookmarkMatch[1])

      if (topicId === undefined) {
        return Effect.succeed(badRequest('topicId is malformed'))
      }

      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return readForumTopicDetail(db, topicId).pipe(
        Effect.flatMap(detail =>
          detail === null
            ? Effect.succeed(notFound())
            : bookmarkForumResponse(
                request,
                db,
                {
                  bookmarkKind: 'topic',
                  forumId: detail.topic.forumId,
                  postId: null,
                  topicId: detail.topic.topicId,
                },
                requestDependencies,
              ),
        ),
        Effect.catch(error => Effect.succeed(writeFailureResponse(error))),
      )
    }

    const topicReportMatch = /^\/api\/forum\/topics\/([^/]+)\/reports$/.exec(
      url.pathname,
    )

    if (topicReportMatch !== null) {
      const topicId = decodePathSegment(topicReportMatch[1])

      if (topicId === undefined) {
        return Effect.succeed(badRequest('topicId is malformed'))
      }

      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return readForumTopicDetail(db, topicId).pipe(
        Effect.flatMap(detail => {
          if (detail === null) {
            return Effect.succeed(notFound())
          }

          return readForumSummaryByRef(db, detail.topic.forumId, {
            allowUnlisted: true,
          }).pipe(
            Effect.flatMap(forum =>
              forum === null
                ? Effect.succeed(notFound())
                : reportForumTargetResponse(
                    request,
                    db,
                    {
                      forumId: forum.forumId,
                      forumSlug: forum.slug,
                      targetId: detail.topic.topicId,
                      targetKind: 'topic',
                    },
                    requestDependencies,
                  ),
            ),
          )
        }),
        Effect.catch(error => Effect.succeed(writeFailureResponse(error))),
      )
    }

    const topicMatch = /^\/api\/forum\/topics\/([^/]+)$/.exec(url.pathname)

    if (topicMatch !== null) {
      const topicId = decodePathSegment(topicMatch[1])

      if (topicId === undefined) {
        return Effect.succeed(badRequest('topicId is malformed'))
      }

      if (request.method === 'PATCH') {
        return editTopicResponse(request, db, topicId, requestDependencies)
      }

      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET', 'PATCH']))
      }

      const postSortDirection = forumTopicPostSortDirectionFromUrl(url)

      return postSortDirection instanceof Response
        ? Effect.succeed(postSortDirection)
        : publicReadResponse(
            readForumTopicDetail(db, topicId, { postSortDirection }),
          )
    }

    const topicPostsMatch = /^\/api\/forum\/topics\/([^/]+)\/posts$/.exec(
      url.pathname,
    )

    if (topicPostsMatch !== null) {
      const topicId = decodePathSegment(topicPostsMatch[1])

      if (topicId === undefined) {
        return Effect.succeed(badRequest('topicId is malformed'))
      }

      return request.method === 'POST'
        ? createReplyResponse(request, db, topicId, requestDependencies)
        : Effect.succeed(methodNotAllowed(['POST']))
    }

    const postBookmarkMatch = /^\/api\/forum\/posts\/([^/]+)\/bookmarks$/.exec(
      url.pathname,
    )

    if (postBookmarkMatch !== null) {
      const postId = decodePathSegment(postBookmarkMatch[1])

      if (postId === undefined) {
        return Effect.succeed(badRequest('postId is malformed'))
      }

      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return readForumPostDetail(db, postId).pipe(
        Effect.flatMap(detail =>
          detail === null
            ? Effect.succeed(notFound())
            : readForumTopicById(db, detail.containingTopicId).pipe(
                Effect.flatMap(topic =>
                  topic === null
                    ? Effect.succeed(serverError())
                    : bookmarkForumResponse(
                        request,
                        db,
                        {
                          bookmarkKind: 'post',
                          forumId: topic.forumId,
                          postId: detail.post.postId,
                          topicId: topic.topicId,
                        },
                        requestDependencies,
                      ),
                ),
              ),
        ),
        Effect.catch(error => Effect.succeed(writeFailureResponse(error))),
      )
    }

    const postReportMatch = /^\/api\/forum\/posts\/([^/]+)\/reports$/.exec(
      url.pathname,
    )

    if (postReportMatch !== null) {
      const postId = decodePathSegment(postReportMatch[1])

      if (postId === undefined) {
        return Effect.succeed(badRequest('postId is malformed'))
      }

      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return readPostControlTarget(db, postId).pipe(
        Effect.flatMap(target =>
          target === null || target.postDetail.post.state === 'tombstoned'
            ? Effect.succeed(notFound())
            : reportForumTargetResponse(
                request,
                db,
                {
                  forumId: target.forum.forumId,
                  forumSlug: target.forum.slug,
                  targetId: target.postDetail.post.postId,
                  targetKind: 'post',
                },
                requestDependencies,
              ),
        ),
        Effect.catch(error => Effect.succeed(writeFailureResponse(error))),
      )
    }

    const postMatch = /^\/api\/forum\/posts\/([^/]+)$/.exec(url.pathname)

    if (postMatch !== null) {
      const postId = decodePathSegment(postMatch[1])

      if (postId === undefined) {
        return Effect.succeed(badRequest('postId is malformed'))
      }

      return request.method === 'GET'
        ? publicReadResponse(
            readForumPostDetail(db, postId).pipe(
              Effect.flatMap(detail =>
                detail === null
                  ? Effect.succeed(null)
                  : readActiveOrangeCheckByActorRef(
                      db,
                      detail.post.author.actorRef,
                      requestDependencies.entitlementsNonGateReads,
                    ).pipe(
                      Effect.map(entitlement => ({
                        ...detail,
                        authorOrangeCheck:
                          orangeCheckBadgeProjection(entitlement),
                      })),
                    ),
              ),
            ),
          )
        : request.method === 'PATCH'
          ? editPostResponse(request, db, postId, requestDependencies)
          : request.method === 'DELETE'
            ? tombstonePostResponse(request, db, postId, requestDependencies)
            : Effect.succeed(methodNotAllowed(['GET', 'PATCH', 'DELETE']))
    }

    return undefined
  },
})
