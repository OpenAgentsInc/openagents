import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import {
  type BuyerPaymentChallengeRecord,
  type BuyerPaymentLedgerAmount,
} from '../buyer-payment-ledger'
import {
  type OpenAgentsHostedMdkClient,
  type OpenAgentsHostedMdkClientError,
  hostedMdkCheckoutRequestFromPaymentChallenge,
  projectOpenAgentsHostedMdkCheckoutResponse,
} from '../hosted-mdk-client'
import { parseJsonUnknown } from '../json-boundary'
import {
  type OpenAgentsL402SigningBoundary,
  l402PayloadFromBuyerPaymentChallenge,
  mintOpenAgentsL402Credential,
} from '../l402-credential-service'
import { formatOpenAgentsL402WwwAuthenticate } from '../l402-payment-headers'
import type { OpenAgentsPaidEndpointProductRecord } from '../paid-endpoint-product-catalog'
import type { PaymentsLedgerDb } from '../payments-ledger-db'
import {
  currentEpochMillis,
  currentIsoTimestamp,
  epochMillisToIsoTimestamp,
  randomUuid,
} from '../runtime-primitives'
import { isTipLadderReceiptRef } from '../tip-ladder'
import {
  mirrorTreasuryRows,
  treasuryAuthorityDb,
  type TreasuryDatabase,
} from '../treasury-domain-store'
import {
  type ForumDirectTipAttemptStatus,
  type ForumDirectTipPaymentEvidence,
  type ForumDirectTipResponse,
  ForumDirectTipResponse as ForumDirectTipResponseSchema,
  type ForumDirectTipWebhookReconciliation,
  ForumDirectTipWebhookReconciliation as ForumDirectTipWebhookReconciliationSchema,
  type ForumL402Challenge,
  ForumL402Challenge as ForumL402ChallengeSchema,
  type ForumL402PaymentChallenge,
  type ForumMoneyAmount,
  type ForumPaidActionKind,
  type ForumPaidActionPreviewResponse,
  ForumPaidActionPreviewResponse as ForumPaidActionPreviewResponseSchema,
  type ForumPaidActionRedeemResponse,
  ForumPaidActionRedeemResponse as ForumPaidActionRedeemResponseSchema,
  type ForumPaidActionTarget,
  type ForumPaymentEventMode,
  type ForumPaymentEventProjection,
  ForumPaymentEventProjection as ForumPaymentEventProjectionSchema,
  type ForumPaymentEventStatus,
  ForumPublicProjectionUnsafe,
  type ForumReceiptLookupResponse,
  ForumReceiptLookupResponse as ForumReceiptLookupResponseSchema,
  type ForumTipSettlementClaimProjection,
  ForumTipSettlementClaimProjection as ForumTipSettlementClaimProjectionSchema,
  type ForumTipSettlementClaimResponse,
  ForumTipSettlementClaimResponse as ForumTipSettlementClaimResponseSchema,
  type ForumTipRecipientDirectPaymentInstruction,
  type ForumTipSettlementProjection,
  type ForumWriteDenialKind,
  decodeForumPublicProjection,
} from './schemas'
import {
  type ForumTipPreviewPolicyDenial,
  ForumTipPreviewRateLimit,
  forumTipImmediatePreviewPolicyDenial,
  forumTipRateLimitPreviewPolicyDenial,
} from './tip-abuse-policy'
import {
  forumTipSettlementProjectionForReceipt,
  forumTipSettlementProjectionForState,
} from './tip-settlement'

export type ForumPaidActionRuntime = Readonly<{
  challengeTtlMs: number
  makeChallengeId: () => string
  makeEntitlementRef: (challengeId: string) => string
  makeMoneyActionId: () => string
  makePaymentEventId: () => string
  makeReceiptId: () => string
  makeReceiptRef: (challengeId: string) => string
  makeRedemptionId: () => string
  makeSettlementClaimId?: () => string
  nowIso: () => string
  nowMillis: () => number
}>

export const systemForumPaidActionRuntime: ForumPaidActionRuntime = {
  challengeTtlMs: 10 * 60_000,
  makeChallengeId: randomUuid,
  makeEntitlementRef: challengeId => `forum_entitlement:${challengeId}`,
  makeMoneyActionId: randomUuid,
  makePaymentEventId: randomUuid,
  makeReceiptId: randomUuid,
  makeReceiptRef: challengeId => `receipt.forum.${challengeId}`,
  makeRedemptionId: randomUuid,
  makeSettlementClaimId: randomUuid,
  nowIso: currentIsoTimestamp,
  nowMillis: currentEpochMillis,
}

export type ForumPaidActionNonPayableDenial = Readonly<{
  denialKind: Exclude<ForumWriteDenialKind, 'payment_required'>
  denialRef: string
  requiredPermission: string | null
}>

export type ForumPaidActionPreviewInput = Readonly<{
  actionKind: ForumPaidActionKind
  actorRef: string
  hostedMdkClient?: OpenAgentsHostedMdkClient | undefined
  idempotencyKey: string
  method: 'GET' | 'POST' | 'PATCH' | 'DELETE'
  nonPayableDenial: ForumPaidActionNonPayableDenial | null
  path: string
  price: ForumMoneyAmount
  publicProjection: unknown
  recipientActorRef: string | null
  recipientReadinessRef: string | null
  requestBodyDigest: string
  routeParams: Readonly<Record<string, string>>
  spendCap: ForumMoneyAmount
  target: ForumPaidActionTarget
}>

export type ForumPaidActionRedeemInput = Readonly<{
  actorRef: string
  challengeId: string
  idempotencyKey: string
  l402ProofRef: string
  method: 'GET' | 'POST' | 'PATCH' | 'DELETE'
  path: string
  paymentEvent?: ForumVerifiedPaymentEventInput | null | undefined
  recipientActorRef: string | null
  recipientReadinessRef: string | null
  requestBodyDigest: string
  routeParams: Readonly<Record<string, string>>
}>

export type ForumPaidActionPrivatePaymentInput = Readonly<{
  actorRef: string
  challengeId: string
  hostedMdkClient?: OpenAgentsHostedMdkClient | undefined
  method: 'GET' | 'POST' | 'PATCH' | 'DELETE'
  path: string
  requestBodyDigest: string
  routeParams: Readonly<Record<string, string>>
  signingBoundary?: OpenAgentsL402SigningBoundary | undefined
  spendCap: ForumMoneyAmount
}>

export type ForumPaidActionPrivatePaymentResponse = Readonly<{
  challenge: ForumL402Challenge
  privatePayment: Readonly<{
    bolt11: string
    checkoutRef: string
    credential: string
    environment: 'production' | 'sandbox'
    expiresAt: string | null
    l402ProofRef: string
    provider: 'mdk_hosted'
    providerRef: string
    sandbox: boolean
  }>
}>

export type ForumTipSettlementClaimInput = Readonly<{
  actorRef: string
  idempotencyKey: string
  receiptRef: string
  settlementEvidenceRefs: ReadonlyArray<string>
  settlementRef: string
  sourceRef: string
}>

export type ForumDirectTipSubmitInput = Readonly<{
  amount: ForumMoneyAmount
  idempotencyKey: string
  payerActorRef: string
  paymentEvidence: ForumDirectTipPaymentEvidence
  post: Readonly<{
    authorActorRef: string
    postId: string
    publicProjection: unknown
    targetPostPermalink: string | null
    topicId: string
  }>
  recipientReadiness: Readonly<{
    directPayment: ForumTipRecipientDirectPaymentInstruction | null
    tippingAvailable: boolean
  }>
}>

export type ForumDirectTipWebhookReconciliationInput = Readonly<{
  amount: ForumMoneyAmount
  attemptId: string
  eventBodyDigestRef: string
  paymentEvidence: ForumDirectTipPaymentEvidence
  providerEventRef: string
  signatureBindingRef: string
}>

export type ForumVerifiedPaymentEventInput = Readonly<{
  externalRef: string
  paymentMode: ForumPaymentEventMode
  providerRef: string
  redactedEvidenceRef: string
  status: ForumPaymentEventStatus
}>

type ChallengeRow = Readonly<{
  action_kind: ForumPaidActionKind
  actor_ref: string
  archived_at: string | null
  created_at: string
  expires_at: string
  id: string
  idempotency_key: string
  method: 'GET' | 'POST' | 'PATCH' | 'DELETE'
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

type CountRow = Readonly<{
  count: number | null
}>

type RedemptionRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  challenge_id: string
  created_at: string
  entitlement_ref: string
  id: string
  receipt_id: string | null
  replayed: number
}>

type ReceiptRow = Readonly<{
  action_kind: ForumPaidActionKind
  amount_asset: 'credits' | 'sats' | 'usd'
  amount_value: number
  created_at: string
  id: string
  public_projection_json: string
  receipt_ref: string
  recipient_actor_ref: string | null
  target_forum_id: string | null
  target_post_id: string | null
  target_topic_id: string | null
}>

type ReceiptLookupRow = ReceiptRow &
  Readonly<{
    payment_event_projection_json: string | null
    settlement_claim_projection_json: string | null
  }>

type TipLadderReceiptLookupRow = Readonly<{
  cost_msat: number
  created_at: string
  credited_through_msat?: number | null
  pay_in_id: string
  public_receipt_ref: string
  payer_ref: string
  payout_external_ref: string | null
  recipient_actor_ref: string
  recipient_swept_msat?: number | null
  rung: 'credited' | 'direct_bolt12' | 'direct_lightning' | null
  state: 'paid' | 'forwarding'
  state_changed_at: string
  target_forum_id: string | null
  target_post_id: string | null
  target_topic_id: string | null
}>

const tipLadderRungIsDirectWallet = (
  rung: TipLadderReceiptLookupRow['rung'],
): boolean => rung === 'direct_bolt12' || rung === 'direct_lightning'

type PaymentEventRow = Readonly<{
  archived_at: string | null
  external_ref: string
  id: string
  money_action_id: string | null
  provider_ref: string
}>

type MoneyActionRow = Readonly<{
  id: string
  payment_event_id: string | null
  public_projection_json: string
  receipt_id: string | null
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
  payment_event_status: ForumPaymentEventStatus
  payment_mode: 'live' | 'sandbox' | 'signet' | 'unknown'
  provider_ref: string
  receipt_ref: string | null
  recipient_actor_ref: string
  redacted_evidence_ref: string
  status: ForumDirectTipAttemptStatus
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
  payment_event_status: ForumPaymentEventStatus
  provider_event_ref: string
  provider_ref: string
  reconciliation_result: string
  reconciliation_status: ForumDirectTipAttemptStatus
  redacted_evidence_ref: string
  signature_binding_ref: string
}>

type SettlementClaimRow = Readonly<{
  archived_at: string | null
  id: string
  idempotency_key: string
  public_projection_json: string
  receipt_id: string
  receipt_ref: string
  recipient_actor_ref: string
}>

export class ForumPaidActionError extends S.TaggedErrorClass<ForumPaidActionError>()(
  'ForumPaidActionError',
  {
    kind: S.Literals([
      'actor_mismatch',
      'binding_mismatch',
      'challenge_expired',
      'challenge_not_found',
      'over_spend_cap',
      'payment_event_replayed',
      'payment_provider_rejected',
      'payment_provider_stale_challenge',
      'payment_provider_unavailable',
      'payment_provider_unconfigured',
      'payment_verification_failed',
      'receipt_not_found',
      'recipient_actor_mismatch',
      'recipient_not_ready',
      'self_tip_blocked',
      'settlement_claim_unavailable',
      'storage_error',
      'unsafe_payment_ref',
    ]),
    reason: S.String,
  },
) {}

const decodeChallenge = S.decodeUnknownSync(ForumL402ChallengeSchema)
const decodePreviewResponse = S.decodeUnknownSync(
  ForumPaidActionPreviewResponseSchema,
)
const decodeRedeemResponse = S.decodeUnknownSync(
  ForumPaidActionRedeemResponseSchema,
)
const decodeReceiptLookup = S.decodeUnknownSync(
  ForumReceiptLookupResponseSchema,
)
const decodeDirectTipResponse = S.decodeUnknownSync(
  ForumDirectTipResponseSchema,
)
const decodeDirectTipWebhookReconciliation = S.decodeUnknownSync(
  ForumDirectTipWebhookReconciliationSchema,
)
const decodePaymentEventProjection = S.decodeUnknownSync(
  ForumPaymentEventProjectionSchema,
)
const decodeSettlementClaimProjection = S.decodeUnknownSync(
  ForumTipSettlementClaimProjectionSchema,
)
const decodeSettlementClaimResponse = S.decodeUnknownSync(
  ForumTipSettlementClaimResponseSchema,
)

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/
const prohibitedPaymentMaterialPattern =
  /(^|\b)(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|raw[_-]?invoice|payment_preimage|preimage|mdk_access_token|wallet_secret|private_key|webhook_secret)/i
const prohibitedSettlementMaterialPattern =
  /(^|\b)(access[_-]?token|api[_-]?key|auth\.json|balance[._-]?sats|bearer|bolt11|bolt12|channel[_-]?monitor|checkout[_-]?secret|cookie|entropy|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|id|preimage|proof=|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(channel|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|backup|balance|channel|invoice|liquidity|payment|payload|payout|target|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(config|key|material|mnemonic|payment|preimage|secret|seed|state))/i
const rawPaymentHashPattern = /^[a-f0-9]{64}$/i
const rawIsoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const refSegmentPattern = /[^A-Za-z0-9_-]+/g

const cleanRefSegment = (value: string): string =>
  value.replace(refSegmentPattern, '_').slice(0, 120)

const challengeRefForId = (challengeId: string): string =>
  `challenge.forum_l402.${cleanRefSegment(challengeId)}`

const credentialRefForChallenge = (challengeId: string): string =>
  `credential.forum_l402.${cleanRefSegment(challengeId)}`

const replayNonceRefForChallenge = (challengeId: string): string =>
  `replay_nonce.forum_l402.${cleanRefSegment(challengeId)}`

const paymentProofRefForChallenge = (challengeId: string): string =>
  `payment_proof.public.forum_reward.${cleanRefSegment(challengeId)}`

const endpointRefForAction = (actionKind: ForumPaidActionKind): string =>
  `endpoint.forum_paid_action.${cleanRefSegment(actionKind)}`

export const ORANGE_CHECK_MDK_PRODUCT_ID = 'cmq7ikvjx00c0ad0yz9sti7qu'

export const forumPaidActionProductId = (actionKind: string): string =>
  actionKind === 'orange_check'
    ? ORANGE_CHECK_MDK_PRODUCT_ID
    : `product.forum.${cleanRefSegment(actionKind)}.single`

const productIdForAction = (actionKind: ForumPaidActionKind): string =>
  forumPaidActionProductId(actionKind)

const entitlementScopeRefForAction = (
  actionKind: ForumPaidActionKind,
): string => `entitlement.forum.${cleanRefSegment(actionKind)}.single`

const idempotencyKeyHashRef = (idempotencyKey: string): string =>
  `sha256:forum_paid_action:${cleanRefSegment(idempotencyKey)}`

const forumAmountToBuyerPaymentAmount = (
  amount: ForumMoneyAmount,
): BuyerPaymentLedgerAmount =>
  amount.asset === 'sats'
    ? {
        amountMinorUnits: amount.amount * 1000,
        asset: 'bitcoin',
        denomination: 'bitcoin_millisatoshi',
      }
    : amount.asset === 'usd'
      ? {
          amountMinorUnits: amount.amount,
          asset: 'usd',
          denomination: 'usd_cent',
        }
      : {
          amountMinorUnits: amount.amount,
          asset: 'credits',
          denomination: 'credit',
        }

const forumPaidActionProduct = (
  input: ForumPaidActionPreviewInput,
): OpenAgentsPaidEndpointProductRecord => ({
  binding: {
    actionRef: `action.forum.${cleanRefSegment(input.actionKind)}`,
    kind: 'forum_paid_action',
    method: input.method,
    pathTemplate: input.path,
    resourceRef: `resource.forum.${cleanRefSegment(input.actionKind)}`,
  },
  displayName: `Forum ${input.actionKind}`,
  entitlement: {
    durationSeconds: null,
    kind: 'resource',
    quotaUnits: null,
    scopeRefs: [entitlementScopeRefForAction(input.actionKind)],
  },
  internalEconomicsRefs: [
    `internal_economics.forum.${cleanRefSegment(input.actionKind)}`,
  ],
  operatorNoteRefs: ['operator_note.forum_paid_action.mdk_l402'],
  price: forumAmountToBuyerPaymentAmount(input.price),
  productId: productIdForAction(input.actionKind),
  projectionPolicy: 'agent_visible',
  providerBindingRefs: ['binding.openagents.hosted_mdk.forum_paid_action'],
  publicAgentDocRefs: ['docs.openagents.forum_paid_actions'],
  publicSummaryRef: `summary.product.forum.${cleanRefSegment(input.actionKind)}`,
  spendCapHintRefs: [
    `spend_cap.forum.${cleanRefSegment(input.actionKind)}.${input.spendCap.asset}_${input.spendCap.amount}`,
  ],
  status: 'active',
  surface: 'forum_paid_action',
})

const buyerPaymentChallengeForForumPreview = (
  input: ForumPaidActionPreviewInput,
  challengeId: string,
  challengeExpiresAt: string,
  runtime: ForumPaidActionRuntime,
): BuyerPaymentChallengeRecord => ({
  actorRef: input.actorRef,
  archivedAt: null,
  challengeRef: challengeRefForId(challengeId),
  createdAt: runtime.nowIso(),
  expiresAt: challengeExpiresAt,
  id: `buyer_payment_challenge.forum.${cleanRefSegment(challengeId)}`,
  idempotencyKeyHash: idempotencyKeyHashRef(input.idempotencyKey),
  metadataRefs: [
    `metadata.forum_paid_action.${cleanRefSegment(input.actionKind)}`,
    ...(input.recipientActorRef === null
      ? []
      : [`recipient.forum_actor.${cleanRefSegment(input.recipientActorRef)}`]),
    ...(input.recipientReadinessRef === null
      ? []
      : [input.recipientReadinessRef]),
  ],
  method: input.method,
  ownerUserId: null,
  path: input.path,
  price: forumAmountToBuyerPaymentAmount(input.price),
  productId: productIdForAction(input.actionKind),
  publicProjectionJson: '{}',
  requestBodyDigest: input.requestBodyDigest,
  spendCap: forumAmountToBuyerPaymentAmount(input.spendCap),
  status: 'issued',
  surface: 'forum_paid_action',
})

const buyerPaymentChallengeForRow = (
  row: ChallengeRow,
): BuyerPaymentChallengeRecord => ({
  actorRef: row.actor_ref,
  archivedAt: row.archived_at,
  challengeRef: challengeRefForId(row.id),
  createdAt: row.created_at,
  expiresAt: row.expires_at,
  id: `buyer_payment_challenge.forum.${cleanRefSegment(row.id)}`,
  idempotencyKeyHash: idempotencyKeyHashRef(row.idempotency_key),
  metadataRefs: [
    `metadata.forum_paid_action.${cleanRefSegment(row.action_kind)}`,
    ...(row.recipient_actor_ref === null
      ? []
      : [`recipient.forum_actor.${cleanRefSegment(row.recipient_actor_ref)}`]),
    ...(row.recipient_readiness_ref === null
      ? []
      : [row.recipient_readiness_ref]),
  ],
  method: row.method,
  ownerUserId: null,
  path: row.path,
  price: forumAmountToBuyerPaymentAmount({
    amount: row.price_value,
    asset: row.price_asset,
  }),
  productId: productIdForAction(row.action_kind),
  publicProjectionJson: row.public_projection_json,
  requestBodyDigest: row.request_body_digest,
  spendCap: forumAmountToBuyerPaymentAmount({
    amount: row.spend_cap_value,
    asset: row.spend_cap_asset,
  }),
  status: 'issued',
  surface: 'forum_paid_action',
})

const stringArrayFromJson = (value: string | null): ReadonlyArray<string> => {
  const parsed = parseJsonUnknown(value ?? '[]')

  return Array.isArray(parsed)
    ? parsed.filter((item): item is string => typeof item === 'string')
    : []
}

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, ForumPaidActionError> =>
  Effect.tryPromise({
    catch: error =>
      new ForumPaidActionError({
        kind: 'storage_error',
        reason:
          error instanceof Error
            ? `${operation}: ${error.message}`
            : `${operation}: ${String(error)}`,
      }),
    try: run,
  })

const validateProjection = (
  projection: unknown,
): Effect.Effect<
  ReturnType<typeof decodeForumPublicProjection>,
  ForumPublicProjectionUnsafe
> =>
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

const validateSpendCap = (
  price: ForumMoneyAmount,
  spendCap: ForumMoneyAmount,
): Effect.Effect<void, ForumPaidActionError> =>
  price.asset === spendCap.asset && price.amount <= spendCap.amount
    ? Effect.void
    : Effect.fail(
        new ForumPaidActionError({
          kind: 'over_spend_cap',
          reason:
            'Forum paid action price must use the spend-cap asset and be within the declared cap.',
        }),
      )

const validatePaymentProofRef = (
  proofRef: string,
): Effect.Effect<void, ForumPaidActionError> =>
  safeRefPattern.test(proofRef) &&
  !containsProviderSecretMaterial(proofRef) &&
  !prohibitedPaymentMaterialPattern.test(proofRef) &&
  !rawPaymentHashPattern.test(proofRef)
    ? Effect.void
    : Effect.fail(
        new ForumPaidActionError({
          kind: 'unsafe_payment_ref',
          reason:
            'Forum L402 proof must be a public-safe redacted proof ref, not raw payment material.',
        }),
      )

const validatePaymentEventRef = (
  label: string,
  value: string,
): Effect.Effect<void, ForumPaidActionError> =>
  safeRefPattern.test(value) &&
  !containsProviderSecretMaterial(value) &&
  !prohibitedPaymentMaterialPattern.test(value) &&
  !rawPaymentHashPattern.test(value)
    ? Effect.void
    : Effect.fail(
        new ForumPaidActionError({
          kind: 'unsafe_payment_ref',
          reason: `${label} must be a public-safe redacted payment event ref.`,
        }),
      )

const validateSettlementRef = (
  label: string,
  value: string,
): Effect.Effect<void, ForumPaidActionError> =>
  safeRefPattern.test(value) &&
  !containsProviderSecretMaterial(value) &&
  !prohibitedSettlementMaterialPattern.test(value) &&
  !rawPaymentHashPattern.test(value) &&
  !rawIsoTimestampPattern.test(value)
    ? Effect.void
    : Effect.fail(
        new ForumPaidActionError({
          kind: 'unsafe_payment_ref',
          reason: `${label} must be a public-safe settlement ref, not wallet, invoice, or raw payment material.`,
        }),
      )

const validateSettlementRefs = (
  input: ForumTipSettlementClaimInput,
): Effect.Effect<void, ForumPaidActionError> =>
  Effect.gen(function* () {
    if (input.settlementEvidenceRefs.length === 0) {
      return yield* new ForumPaidActionError({
        kind: 'unsafe_payment_ref',
        reason:
          'Forum settlement claims require at least one public-safe settlement evidence ref.',
      })
    }

    if (input.settlementEvidenceRefs.length > 10) {
      return yield* new ForumPaidActionError({
        kind: 'unsafe_payment_ref',
        reason: 'Forum settlement claims can attach at most 10 evidence refs.',
      })
    }

    yield* validateSettlementRef('settlementRef', input.settlementRef)
    yield* validateSettlementRef('sourceRef', input.sourceRef)
    for (const [index, ref] of input.settlementEvidenceRefs.entries()) {
      yield* validateSettlementRef(`settlementEvidenceRefs[${index}]`, ref)
    }
  })

const validatePaymentEventRefs = (
  event: ForumVerifiedPaymentEventInput | ForumDirectTipPaymentEvidence,
): Effect.Effect<void, ForumPaidActionError> =>
  Effect.gen(function* () {
    yield* validatePaymentEventRef('providerRef', event.providerRef)
    yield* validatePaymentEventRef('externalRef', event.externalRef)
    yield* validatePaymentEventRef(
      'redactedEvidenceRef',
      event.redactedEvidenceRef,
    )
  })

const validateVerifiedPaymentEvent = (
  event: ForumVerifiedPaymentEventInput | null | undefined,
): Effect.Effect<void, ForumPaidActionError> =>
  Effect.gen(function* () {
    if (event === null || event === undefined) {
      return
    }

    yield* validatePaymentEventRefs(event)

    if (event.status !== 'confirmed') {
      return yield* new ForumPaidActionError({
        kind: 'payment_verification_failed',
        reason:
          'Forum payment event must be confirmed before reward redemption can link it.',
      })
    }
  })

const directTipStatusForPaymentEvent = (
  status: ForumPaymentEventStatus,
): ForumDirectTipAttemptStatus =>
  status === 'confirmed'
    ? 'settled'
    : status === 'failed' || status === 'refunded' || status === 'reversed'
      ? 'failed'
      : 'recovery_pending'

const validateDirectTipSubmitInput = (
  input: ForumDirectTipSubmitInput,
): Effect.Effect<void, ForumPaidActionError | ForumPublicProjectionUnsafe> =>
  Effect.gen(function* () {
    yield* validateProjection(input.post.publicProjection)
    yield* validatePaymentEventRefs(input.paymentEvidence)

    if (input.amount.asset !== 'sats' || input.amount.amount <= 0) {
      return yield* new ForumPaidActionError({
        kind: 'over_spend_cap',
        reason:
          'Forum direct tips must be positive sats amounts selected by the payer.',
      })
    }

    if (input.payerActorRef === input.post.authorActorRef) {
      return yield* new ForumPaidActionError({
        kind: 'self_tip_blocked',
        reason: 'Forum direct tips cannot target a post by the same actor.',
      })
    }

    if (
      !input.recipientReadiness.tippingAvailable ||
      input.recipientReadiness.directPayment?.kind !== 'bolt12_offer'
    ) {
      return yield* new ForumPaidActionError({
        kind: 'recipient_not_ready',
        reason:
          'Forum direct tips require target author readiness with a public BOLT 12 offer.',
      })
    }
  })

const stableJson = (value: Readonly<Record<string, string>>): string =>
  JSON.stringify(
    Object.fromEntries(
      Object.entries(value).sort(([left], [right]) =>
        left.localeCompare(right),
      ),
    ),
  )

const expiresAt = (runtime: ForumPaidActionRuntime): string =>
  epochMillisToIsoTimestamp(runtime.nowMillis() + runtime.challengeTtlMs)

const paidActionErrorKindFromMdkError = (
  error: OpenAgentsHostedMdkClientError,
): ForumPaidActionError['kind'] =>
  error.reason === 'missing_configuration'
    ? 'payment_provider_unconfigured'
    : error.reason === 'provider_unavailable'
      ? 'payment_provider_unavailable'
      : error.reason === 'stale_challenge'
        ? 'payment_provider_stale_challenge'
        : 'payment_provider_rejected'

const paidActionErrorFromMdkError = (
  error: OpenAgentsHostedMdkClientError,
): ForumPaidActionError =>
  new ForumPaidActionError({
    kind: paidActionErrorKindFromMdkError(error),
    reason: error.detailRef,
  })

const validateRewardRecipientReadiness = (
  input: ForumPaidActionPreviewInput,
): Effect.Effect<void, ForumPaidActionError> =>
  input.actionKind === 'post_reward' &&
  input.recipientActorRef !== null &&
  input.recipientReadinessRef === null
    ? Effect.fail(
        new ForumPaidActionError({
          kind: 'recipient_not_ready',
          reason:
            'Forum post rewards require a ready recipient wallet readiness ref before issuing an MDK/L402 challenge.',
        }),
      )
    : Effect.void

const directTipRequiredPreviewResponse = (
  actorRef: string,
): ForumPaidActionPreviewResponse =>
  decodePreviewResponse({
    challenge: null,
    entitlementRef: null,
    paymentRequired: false,
    writeDenial: {
      actorRef,
      denialKind: 'payment_required',
      denialRef: 'blocker.public.forum_tip.bolt12_direct_required',
      payable: false,
      requiredPermission: null,
    },
  })

const deniedPreviewResponse = (
  actorRef: string,
  denial: ForumPaidActionNonPayableDenial | ForumTipPreviewPolicyDenial,
): ForumPaidActionPreviewResponse =>
  decodePreviewResponse({
    challenge: null,
    entitlementRef: null,
    paymentRequired: false,
    writeDenial: {
      actorRef,
      denialKind: denial.denialKind,
      denialRef: denial.denialRef,
      payable: false,
      requiredPermission: denial.requiredPermission,
    },
  })

const issueForumMdkL402Challenge = (
  input: ForumPaidActionPreviewInput,
  challengeId: string,
  challengeExpiresAt: string,
  runtime: ForumPaidActionRuntime,
): Effect.Effect<ForumL402PaymentChallenge, ForumPaidActionError> =>
  Effect.gen(function* () {
    if (input.hostedMdkClient === undefined) {
      return yield* new ForumPaidActionError({
        kind: 'payment_provider_unconfigured',
        reason: 'detail.forum_paid_action.hosted_mdk_client_missing',
      })
    }

    const challenge = buyerPaymentChallengeForForumPreview(
      input,
      challengeId,
      challengeExpiresAt,
      runtime,
    )
    const product = forumPaidActionProduct(input)
    const endpointRef = endpointRefForAction(input.actionKind)
    const entitlementScopeRefs = [
      entitlementScopeRefForAction(input.actionKind),
    ]
    const credentialRef = credentialRefForChallenge(challengeId)
    const replayNonceRef = replayNonceRefForChallenge(challengeId)
    const pendingL402Payload = l402PayloadFromBuyerPaymentChallenge({
      challenge,
      credentialRef,
      endpointRef,
      entitlementScopeRefs,
      issuedAt: runtime.nowIso(),
      paymentHashRef: `payment_hash.redacted.pending.${cleanRefSegment(challengeId)}`,
      replayNonceRef,
    })
    const checkoutRequest = yield* hostedMdkCheckoutRequestFromPaymentChallenge(
      {
        cancelRef: `return.cancel.forum.${cleanRefSegment(input.actionKind)}`,
        challenge,
        environment:
          input.hostedMdkClient.implementationState === 'fake_provider_contract'
            ? 'sandbox'
            : 'production',
        l402Payload: pendingL402Payload,
        metadataRefs: [
          `metadata.forum_l402.${cleanRefSegment(input.actionKind)}`,
        ],
        mode: 'l402_invoice',
        product,
        returnRef: `return.success.forum.${cleanRefSegment(input.actionKind)}`,
        sandbox:
          input.hostedMdkClient.implementationState !==
          'live_provider_configured',
        siteRef: null,
      },
    ).pipe(Effect.mapError(paidActionErrorFromMdkError))
    const checkout = yield* input.hostedMdkClient
      .createCheckout(checkoutRequest)
      .pipe(Effect.mapError(paidActionErrorFromMdkError))
    const checkoutProjection = projectOpenAgentsHostedMdkCheckoutResponse(
      checkout,
      'agent',
    )
    const wwwAuthenticate = formatOpenAgentsL402WwwAuthenticate({
      amount: challenge.price,
      challengeRef: challenge.challengeRef,
      docsRef: 'docs.openagents.forum_paid_actions',
      endpointRef,
      expiresAt: challenge.expiresAt,
      productId: product.productId,
    })

    return {
      acceptedWorkSettlementAuthority: false,
      checkoutLaunchPath: checkoutProjection.checkoutLaunchPath ?? null,
      checkoutRef: checkoutProjection.checkoutRef,
      checkoutUrlRef: checkoutProjection.checkoutUrlRef,
      credentialRef,
      endpointRef,
      entitlementScopeRefs,
      environment: checkoutProjection.environment,
      implementationState: input.hostedMdkClient.implementationState,
      invoiceRef: checkoutProjection.invoiceRef,
      paymentHashRef: checkoutProjection.paymentHashRef,
      provider: 'mdk_hosted',
      providerMode: 'hosted_mdk',
      providerPayoutAuthority: false,
      providerRef: checkoutProjection.providerRef,
      replayNonceRef,
      sandbox: checkoutProjection.sandbox,
      settlementAuthority: 'buyer_payment_evidence_only',
      wwwAuthenticate,
    }
  })

const routeParamsMatch = (
  left: string,
  right: Readonly<Record<string, string>>,
): boolean => left === stableJson(right)

const l402ChallengeFromRow = (
  row: ChallengeRow,
): ForumL402PaymentChallenge | null =>
  row.l402_credential_ref === null ||
  row.l402_replay_nonce_ref === null ||
  row.l402_endpoint_ref === null ||
  row.l402_www_authenticate === null ||
  row.mdk_provider_ref === null ||
  row.mdk_environment === null ||
  row.mdk_sandbox === null ||
  row.mdk_implementation_state === null
    ? null
    : {
        acceptedWorkSettlementAuthority: false,
        checkoutLaunchPath: row.mdk_checkout_launch_path,
        checkoutRef: row.mdk_checkout_ref,
        checkoutUrlRef: row.mdk_checkout_url_ref,
        credentialRef: row.l402_credential_ref,
        endpointRef: row.l402_endpoint_ref,
        entitlementScopeRefs: stringArrayFromJson(
          row.l402_entitlement_scope_refs_json,
        ),
        environment: row.mdk_environment,
        implementationState: row.mdk_implementation_state,
        invoiceRef: row.mdk_invoice_ref,
        paymentHashRef: row.mdk_payment_hash_ref,
        provider: 'mdk_hosted',
        providerMode: 'hosted_mdk',
        providerPayoutAuthority: false,
        providerRef: row.mdk_provider_ref,
        replayNonceRef: row.l402_replay_nonce_ref,
        sandbox: row.mdk_sandbox === 1,
        settlementAuthority: 'buyer_payment_evidence_only',
        wwwAuthenticate: row.l402_www_authenticate,
      }

const challengeFromRow = (row: ChallengeRow): ForumL402Challenge =>
  decodeChallenge({
    actionKind: row.action_kind,
    actorRef: row.actor_ref,
    challengeId: row.id,
    expiresAt: row.expires_at,
    l402: l402ChallengeFromRow(row),
    method: row.method,
    path: row.path,
    price: {
      amount: row.price_value,
      asset: row.price_asset,
    },
    recipientActorRef: row.recipient_actor_ref,
    recipientReadinessRef: row.recipient_readiness_ref,
    requestBodyDigest: row.request_body_digest,
    routeParams: parseJsonUnknown(row.route_params_json),
    spendCap: {
      amount: row.spend_cap_value,
      asset: row.spend_cap_asset,
    },
    target: {
      forumId: row.target_forum_id,
      postId: row.target_post_id,
      topicId: row.target_topic_id,
    },
  })

const paymentEventProjectionFromRow = (
  row: ReceiptLookupRow,
): ForumPaymentEventProjection | null => {
  if (row.payment_event_projection_json === null) {
    return null
  }

  return decodePaymentEventProjection(
    parseJsonUnknown(row.payment_event_projection_json),
  )
}

const settlementClaimProjectionFromRow = (
  row: ReceiptLookupRow,
): ForumTipSettlementClaimProjection | null => {
  if (row.settlement_claim_projection_json == null) {
    return null
  }

  return decodeSettlementClaimProjection(
    parseJsonUnknown(row.settlement_claim_projection_json),
  )
}

const forumTopicPublicUrl = (topicId: string): string =>
  `https://openagents.com/forum/t/${encodeURIComponent(topicId)}`

const forumPostPublicUrl = (topicId: string, postId: string): string =>
  `${forumTopicPublicUrl(topicId)}#post-${encodeURIComponent(postId)}`

const receiptTargetPostPermalink = (row: ReceiptLookupRow): string | null =>
  row.target_topic_id === null || row.target_post_id === null
    ? null
    : forumPostPublicUrl(row.target_topic_id, row.target_post_id)

const receiptLookupFromRow = (
  row: ReceiptLookupRow,
): ForumReceiptLookupResponse => {
  const paymentEvent = paymentEventProjectionFromRow(row)
  const settlementClaim = settlementClaimProjectionFromRow(row)

  return decodeReceiptLookup({
    actionKind: row.action_kind,
    amount: {
      amount: row.amount_value,
      asset: row.amount_asset,
    },
    createdAt: row.created_at,
    paymentEvent,
    publicProjection: decodeForumPublicProjection(
      parseJsonUnknown(row.public_projection_json),
    ),
    receiptRef: row.receipt_ref,
    recipientActorRef: row.recipient_actor_ref,
    target: {
      forumId: row.target_forum_id,
      postId: row.target_post_id,
      topicId: row.target_topic_id,
    },
    targetPostPermalink: receiptTargetPostPermalink(row),
    settlementClaim,
    tipSettlement: forumTipSettlementProjectionForReceipt(
      paymentEvent,
      settlementClaim,
    ),
  })
}

const tipLadderTargetPostPermalink = (
  row: TipLadderReceiptLookupRow,
): string | null =>
  row.target_topic_id === null || row.target_post_id === null
    ? null
    : forumPostPublicUrl(row.target_topic_id, row.target_post_id)

const tipLadderPaymentEventFromRow = (
  row: TipLadderReceiptLookupRow,
): ForumPaymentEventProjection =>
  decodePaymentEventProjection({
    actionKind: 'post_reward',
    amount: {
      amount: Math.max(0, Math.floor(Number(row.cost_msat) / 1000)),
      asset: 'sats',
    },
    challengeId: row.pay_in_id,
    createdAt: row.state_changed_at,
    externalRef:
      tipLadderRungIsDirectWallet(row.rung)
        ? `payment.forum.tip_ladder.${row.pay_in_id}`
        : `ledger.forum.tip_ladder.${row.pay_in_id}`,
    payerActorRef: row.payer_ref,
    paymentEventRef: `payment_event.forum.tip_ladder.${row.pay_in_id}`,
    paymentMode: 'live',
    providerRef: 'provider.openagents.tip_ladder',
    receiptRef: row.public_receipt_ref,
    recipientActorRef: row.recipient_actor_ref,
    redactedEvidenceRef: `evidence.forum.tip_ladder.${row.pay_in_id}`,
    settlementAuthority:
      row.state === 'paid' && tipLadderRungIsDirectWallet(row.rung)
        ? 'recipient_wallet_direct'
        : row.state === 'paid' && row.rung === 'credited'
          ? 'openagents_ledger_credited'
          : 'buyer_payment_evidence_only',
    status: row.state === 'paid' ? 'confirmed' : 'observed',
  })

// Swept coverage for the receipt lookup (#4753): a paid credited-rung
// tip whose cumulative credited value is covered by settled sweep
// payouts (oldest-credited-first) reads as 'swept', so sweep
// completion transitions the public bucket instead of freezing it.
const tipLadderSettlementFromRow = (
  row: TipLadderReceiptLookupRow,
  paymentEvent: ForumPaymentEventProjection,
): ForumTipSettlementProjection => {
  const creditedThroughMsat = Math.max(
    0,
    Number(row.credited_through_msat ?? 0),
  )
  const recipientSweptMsat = Math.max(0, Number(row.recipient_swept_msat ?? 0))

  return row.state === 'paid' &&
    row.rung === 'credited' &&
    creditedThroughMsat > 0 &&
    recipientSweptMsat >= creditedThroughMsat
    ? forumTipSettlementProjectionForState('swept')
    : forumTipSettlementProjectionForReceipt(paymentEvent, null)
}

const tipLadderReceiptLookupFromRow = (
  row: TipLadderReceiptLookupRow,
): ForumReceiptLookupResponse => {
  const paymentEvent = tipLadderPaymentEventFromRow(row)

  return decodeReceiptLookup({
    actionKind: 'post_reward',
    amount: {
      amount: Math.max(0, Math.floor(Number(row.cost_msat) / 1000)),
      asset: 'sats',
    },
    createdAt: row.created_at,
    paymentEvent,
    publicProjection: {
      classificationCaveatRef:
        'caveat.public.forum_tip_ladder.receipt_redacted',
      customerSafe: true,
      dataClassification: 'public',
      excludedPrivateRefs: [
        'pay_ins.idempotency_key',
        'pay_in_legs.external_ref.raw',
      ],
      publicSafe: true,
      redactionPolicyRef: 'redaction.public.forum_tip_ladder.receipt.v1',
      safeArtifactRefs: [`pay_in.public.${row.pay_in_id}`],
      safeReceiptRefs: [row.public_receipt_ref],
      trustTier: 'verified',
    },
    receiptRef: row.public_receipt_ref,
    recipientActorRef: row.recipient_actor_ref,
    target: {
      forumId: row.target_forum_id,
      postId: row.target_post_id,
      topicId: row.target_topic_id,
    },
    targetPostPermalink: tipLadderTargetPostPermalink(row),
    settlementClaim: null,
    tipSettlement: tipLadderSettlementFromRow(row, paymentEvent),
  })
}

const settlementClaimProjection = ({
  claimId,
  createdAt,
  input,
}: Readonly<{
  claimId: string
  createdAt: string
  input: ForumTipSettlementClaimInput
}>): ForumTipSettlementClaimProjection =>
  decodeSettlementClaimProjection({
    claimId,
    createdAt,
    receiptRef: input.receiptRef,
    recipientActorRef: input.actorRef,
    settlementEvidenceRefs: [...input.settlementEvidenceRefs],
    settlementRef: input.settlementRef,
    sourceRef: input.sourceRef,
  })

const paymentEventProjection = ({
  challenge,
  event,
  eventId,
  input,
  receiptRef,
  runtime,
}: Readonly<{
  challenge: ChallengeRow
  event: ForumVerifiedPaymentEventInput
  eventId: string
  input: ForumPaidActionRedeemInput
  receiptRef: string
  runtime: ForumPaidActionRuntime
}>): ForumPaymentEventProjection =>
  decodePaymentEventProjection({
    actionKind: challenge.action_kind,
    amount: {
      amount: challenge.price_value,
      asset: challenge.price_asset,
    },
    challengeId: challenge.id,
    createdAt: runtime.nowIso(),
    externalRef: event.externalRef,
    payerActorRef: input.actorRef,
    paymentEventRef: eventId,
    paymentMode: event.paymentMode,
    providerRef: event.providerRef,
    receiptRef,
    recipientActorRef: input.recipientActorRef,
    redactedEvidenceRef: event.redactedEvidenceRef,
    settlementAuthority: 'buyer_payment_evidence_only',
    status: event.status,
  })

const directTipPaymentEventProjection = ({
  amount,
  attemptId,
  eventId,
  input,
  receiptRef,
  runtime,
}: Readonly<{
  amount: ForumMoneyAmount
  attemptId: string
  eventId: string
  input: ForumDirectTipSubmitInput
  receiptRef: string | null
  runtime: ForumPaidActionRuntime
}>): ForumPaymentEventProjection =>
  decodePaymentEventProjection({
    actionKind: 'post_reward',
    amount,
    challengeId: attemptId,
    createdAt: runtime.nowIso(),
    externalRef: input.paymentEvidence.externalRef,
    payerActorRef: input.payerActorRef,
    paymentEventRef: eventId,
    paymentMode: input.paymentEvidence.paymentMode,
    providerRef: input.paymentEvidence.providerRef,
    receiptRef,
    recipientActorRef: input.post.authorActorRef,
    redactedEvidenceRef: input.paymentEvidence.redactedEvidenceRef,
    settlementAuthority: 'recipient_wallet_direct',
    status: input.paymentEvidence.status,
  })

const directTipWebhookPaymentEventProjection = ({
  amount,
  attempt,
  eventId,
  paymentEvidence,
  receiptRef,
  runtime,
}: Readonly<{
  amount: ForumMoneyAmount
  attempt: DirectTipAttemptRow
  eventId: string
  paymentEvidence: ForumDirectTipPaymentEvidence
  receiptRef: string | null
  runtime: ForumPaidActionRuntime
}>): ForumPaymentEventProjection =>
  decodePaymentEventProjection({
    actionKind: 'post_reward',
    amount,
    challengeId: attempt.id,
    createdAt: runtime.nowIso(),
    externalRef: paymentEvidence.externalRef,
    payerActorRef: attempt.payer_actor_ref,
    paymentEventRef: eventId,
    paymentMode: paymentEvidence.paymentMode,
    providerRef: paymentEvidence.providerRef,
    receiptRef,
    recipientActorRef: attempt.recipient_actor_ref,
    redactedEvidenceRef: paymentEvidence.redactedEvidenceRef,
    settlementAuthority: 'recipient_wallet_direct',
    status: paymentEvidence.status,
  })

const directTipResponse = (
  attempt: DirectTipAttemptRow,
  receipt: ForumReceiptLookupResponse | null,
  idempotent: boolean,
): ForumDirectTipResponse =>
  decodeDirectTipResponse({
    amount: {
      amount: attempt.amount_sats,
      asset: 'sats',
    },
    attemptId: attempt.id,
    idempotent,
    payerActorRef: attempt.payer_actor_ref,
    paymentEvidence: {
      externalRef: attempt.external_ref,
      paymentMode: attempt.payment_mode,
      providerRef: attempt.provider_ref,
      redactedEvidenceRef: attempt.redacted_evidence_ref,
      status: attempt.payment_event_status,
    },
    postId: attempt.target_post_id,
    receipt,
    recipientActorRef: attempt.recipient_actor_ref,
    status: attempt.status,
    targetPostPermalink: attempt.target_post_permalink,
  })

const insertChallenge = (
  db: D1Database,
  input: ForumPaidActionPreviewInput,
  challengeId: string,
  l402: ForumL402PaymentChallenge,
  projectionJson: string,
  challengeExpiresAt: string,
  runtime: ForumPaidActionRuntime,
): Effect.Effect<void, ForumPaidActionError> =>
  d1Effect('forumPaidActions.insertChallenge', () =>
    db
      .prepare(
        `INSERT OR IGNORE INTO forum_l402_challenges (
           id,
           idempotency_key,
           actor_ref,
           action_kind,
           method,
           path,
           route_params_json,
           request_body_digest,
           target_forum_id,
           target_topic_id,
           target_post_id,
           recipient_actor_ref,
           recipient_readiness_ref,
           price_asset,
           price_value,
           spend_cap_asset,
           spend_cap_value,
           expires_at,
           mdk_provider_ref,
           mdk_environment,
           mdk_sandbox,
           mdk_implementation_state,
           mdk_checkout_ref,
           mdk_checkout_url_ref,
           mdk_checkout_launch_path,
           mdk_invoice_ref,
           mdk_payment_hash_ref,
           l402_credential_ref,
           l402_replay_nonce_ref,
           l402_endpoint_ref,
           l402_entitlement_scope_refs_json,
           l402_www_authenticate,
           public_projection_json,
           created_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        challengeId,
        input.idempotencyKey,
        input.actorRef,
        input.actionKind,
        input.method,
        input.path,
        stableJson(input.routeParams),
        input.requestBodyDigest,
        input.target.forumId,
        input.target.topicId,
        input.target.postId,
        input.recipientActorRef,
        input.recipientReadinessRef,
        input.price.asset,
        input.price.amount,
        input.spendCap.asset,
        input.spendCap.amount,
        challengeExpiresAt,
        l402.providerRef,
        l402.environment,
        l402.sandbox ? 1 : 0,
        l402.implementationState,
        l402.checkoutRef,
        l402.checkoutUrlRef,
        l402.checkoutLaunchPath,
        l402.invoiceRef,
        l402.paymentHashRef,
        l402.credentialRef,
        l402.replayNonceRef,
        l402.endpointRef,
        JSON.stringify(l402.entitlementScopeRefs),
        l402.wwwAuthenticate,
        projectionJson,
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.asVoid)

const readChallengeById = (
  db: D1Database,
  challengeId: string,
): Effect.Effect<ChallengeRow | null, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readChallengeById', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_l402_challenges
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(challengeId)
      .first<ChallengeRow>(),
  )

const readChallengeByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<ChallengeRow | null, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readChallengeByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_l402_challenges
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<ChallengeRow>(),
  )

const readRecentChallengeCountForActor = (
  db: D1Database,
  input: Readonly<{
    actionKind: ForumPaidActionKind
    actorRef: string
    sinceIso: string
  }>,
): Effect.Effect<number, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readRecentChallengeCountForActor', () =>
    db
      .prepare(
        `SELECT COUNT(*) AS count
           FROM forum_l402_challenges
          WHERE actor_ref = ?
            AND action_kind = ?
            AND created_at >= ?
            AND archived_at IS NULL`,
      )
      .bind(input.actorRef, input.actionKind, input.sinceIso)
      .first<CountRow>(),
  ).pipe(Effect.map(row => Math.max(0, Number(row?.count ?? 0))))

const readRedemptionByChallengeId = (
  db: D1Database,
  challengeId: string,
): Effect.Effect<RedemptionRow | null, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readRedemptionByChallengeId', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_l402_redemptions
          WHERE challenge_id = ?
            AND archived_at IS NULL
          ORDER BY created_at ASC
          LIMIT 1`,
      )
      .bind(challengeId)
      .first<RedemptionRow>(),
  )

const readReceiptById = (
  db: D1Database,
  receiptId: string,
): Effect.Effect<ReceiptRow | null, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readReceiptById', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_receipts
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(receiptId)
      .first<ReceiptRow>(),
  )

const readReceiptLookupRowByRef = (
  db: D1Database,
  receiptRef: string,
): Effect.Effect<ReceiptLookupRow | null, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readReceiptLookupRowByRef', () =>
    db
      .prepare(
        `SELECT r.*,
                pe.public_projection_json AS payment_event_projection_json,
                sc.public_projection_json AS settlement_claim_projection_json
           FROM forum_receipts r
           LEFT JOIN forum_money_actions ma
             ON ma.receipt_id = r.id
            AND ma.archived_at IS NULL
           LEFT JOIN forum_payment_events pe
             ON pe.id = ma.payment_event_id
            AND pe.archived_at IS NULL
           LEFT JOIN forum_tip_settlement_claims sc
             ON sc.receipt_id = r.id
            AND sc.archived_at IS NULL
          WHERE r.receipt_ref = ?
            AND r.archived_at IS NULL
          LIMIT 1`,
      )
      .bind(receiptRef)
      .first<ReceiptLookupRow>(),
  )

const readTipLadderReceiptLookupRowByRef = (
  // CFG-4 (#8519): pay_ins/pay_in_legs are Cloud SQL Postgres-authoritative,
  // so this WHOLE lookup (including its `forum_posts` join) runs on the
  // ledger handle. `forum_posts` has a Postgres twin in the same database,
  // converged by the always-on KS-8.10 dual-write — this cross-domain read
  // therefore reads the forum_posts Postgres twin, not the D1 authority.
  ledgerDb: PaymentsLedgerDb,
  receiptRef: string,
): Effect.Effect<TipLadderReceiptLookupRow | null, ForumPaidActionError> =>
  isTipLadderReceiptRef(receiptRef)
    ? d1Effect('forumPaidActions.readTipLadderReceiptLookupRowByRef', () =>
        ledgerDb.query(
          // The ref resolves either as the stored public receipt ref
          // or as the deterministic receipt-equivalent ref
          // 'receipt.forum.tip_ladder.payin.<payInId>' projected for
          // ladder rows that predate the stored column (#4753).
          `SELECT p.id AS pay_in_id,
                    COALESCE(
                      p.public_receipt_ref,
                      'receipt.forum.tip_ladder.payin.' || p.id
                    ) AS public_receipt_ref,
                    p.payer_ref AS payer_ref,
                    p.cost_msat AS cost_msat,
                    p.state AS state,
                    p.rung AS rung,
                    p.created_at AS created_at,
                    p.state_changed_at AS state_changed_at,
                    payout.party_ref AS recipient_actor_ref,
                    payout.external_ref AS payout_external_ref,
                    CASE WHEN p.rung = 'credited' AND p.state = 'paid' THEN (
                      SELECT COALESCE(SUM(p2.cost_msat), 0)
                        FROM pay_ins p2
                        JOIN pay_in_legs payout2
                          ON payout2.pay_in_id = p2.id
                         AND payout2.direction = 'out'
                         AND payout2.party_ref = payout.party_ref
                       WHERE p2.pay_in_type = 'tip'
                         AND p2.rung = 'credited'
                         AND p2.state = 'paid'
                         AND p2.context_ref LIKE 'forum.post.%'
                         AND (p2.created_at < p.created_at
                              OR (p2.created_at = p.created_at
                                  AND p2.id <= p.id))
                    ) ELSE 0 END AS credited_through_msat,
                    (SELECT COALESCE(SUM(s.cost_msat), 0)
                       FROM pay_ins s
                      WHERE s.pay_in_type = 'sweep'
                        AND s.state = 'paid'
                        AND s.payer_ref = payout.party_ref
                    ) AS recipient_swept_msat,
                    forum_posts.id AS target_post_id,
                    forum_posts.topic_id AS target_topic_id,
                    forum_posts.forum_id AS target_forum_id
               FROM pay_ins p
               JOIN pay_in_legs payout
                 ON payout.pay_in_id = p.id
                AND payout.direction = 'out'
          LEFT JOIN forum_posts
                 ON forum_posts.id = substr(
                      p.context_ref,
                      length('forum.post.') + 1
                    )
                AND forum_posts.archived_at IS NULL
              WHERE p.pay_in_type = 'tip'
                AND (p.public_receipt_ref = ?
                     OR (p.public_receipt_ref IS NULL
                         AND 'receipt.forum.tip_ladder.payin.' || p.id = ?))
                AND p.state IN ('paid', 'forwarding')
                AND p.context_ref LIKE 'forum.post.%'
              ORDER BY CASE WHEN p.state = 'paid' THEN 0 ELSE 1 END,
                       p.created_at DESC,
                       p.id DESC
              LIMIT 1`,
          [receiptRef, receiptRef],
        ),
      ).pipe(
        Effect.map(
          rows =>
            (rows[0] as unknown as TipLadderReceiptLookupRow | undefined) ??
            null,
        ),
      )
    : Effect.succeed(null)

const receiptStatement = (
  db: D1Database,
  challenge: ChallengeRow,
  input: ForumPaidActionRedeemInput,
  receiptId: string,
  receiptRef: string,
  runtime: ForumPaidActionRuntime,
): D1PreparedStatement =>
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
      receiptId,
      receiptRef,
      challenge.action_kind,
      challenge.target_forum_id,
      challenge.target_topic_id,
      challenge.target_post_id,
      challenge.price_asset,
      challenge.price_value,
      input.recipientActorRef,
      input.l402ProofRef,
      challenge.public_projection_json,
      runtime.nowIso(),
    )

const moneyActionStatement = (
  db: D1Database,
  challenge: ChallengeRow,
  input: ForumPaidActionRedeemInput,
  moneyActionId: string,
  paymentEventId: string | null,
  receiptId: string,
  runtime: ForumPaidActionRuntime,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT OR IGNORE INTO forum_money_actions (
           id,
           idempotency_key,
           actor_ref,
           action_kind,
           target_forum_id,
           target_topic_id,
           target_post_id,
           amount_asset,
           amount_value,
           payment_event_id,
           receipt_id,
           earning_actor_ref,
           public_projection_json,
           created_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      moneyActionId,
      `money:${input.idempotencyKey}`,
      input.actorRef,
      challenge.action_kind,
      challenge.target_forum_id,
      challenge.target_topic_id,
      challenge.target_post_id,
      challenge.price_asset,
      challenge.price_value,
      paymentEventId,
      receiptId,
      input.recipientActorRef,
      challenge.public_projection_json,
      runtime.nowIso(),
    )

const readPaymentEventByProviderExternal = (
  db: D1Database,
  providerRef: string,
  externalRef: string,
): Effect.Effect<PaymentEventRow | null, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readPaymentEventByProviderExternal', () =>
    db
      .prepare(
        `SELECT id,
                money_action_id,
                provider_ref,
                external_ref,
                archived_at
           FROM forum_payment_events
          WHERE provider_ref = ?
            AND external_ref = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(providerRef, externalRef)
      .first<PaymentEventRow>(),
  )

const paymentEventStatement = (
  db: D1Database,
  challenge: ChallengeRow,
  input: ForumPaidActionRedeemInput,
  event: ForumVerifiedPaymentEventInput,
  eventId: string,
  moneyActionId: string,
  receiptRef: string,
  runtime: ForumPaidActionRuntime,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT INTO forum_payment_events (
           id,
           money_action_id,
           provider_ref,
           external_ref,
           amount_asset,
           amount_value,
           redacted_evidence_ref,
           public_projection_json,
           created_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      eventId,
      moneyActionId,
      event.providerRef,
      event.externalRef,
      challenge.price_asset,
      challenge.price_value,
      event.redactedEvidenceRef,
      JSON.stringify(
        paymentEventProjection({
          challenge,
          event,
          eventId,
          input,
          receiptRef,
          runtime,
        }),
      ),
      runtime.nowIso(),
    )

const readDirectTipAttemptById = (
  db: D1Database,
  attemptId: string,
): Effect.Effect<DirectTipAttemptRow | null, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readDirectTipAttemptById', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_direct_tip_attempts
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(attemptId)
      .first<DirectTipAttemptRow>(),
  )

const readDirectTipAttemptByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<DirectTipAttemptRow | null, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readDirectTipAttemptByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_direct_tip_attempts
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<DirectTipAttemptRow>(),
  )

const readDirectTipAttemptByProviderExternal = (
  db: D1Database,
  providerRef: string,
  externalRef: string,
): Effect.Effect<DirectTipAttemptRow | null, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readDirectTipAttemptByProviderExternal', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_direct_tip_attempts
          WHERE provider_ref = ?
            AND external_ref = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(providerRef, externalRef)
      .first<DirectTipAttemptRow>(),
  )

const readDirectTipWebhookEventByProviderEventRef = (
  db: D1Database,
  providerEventRef: string,
): Effect.Effect<DirectTipWebhookEventRow | null, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readDirectTipWebhookEventByProviderEventRef', () =>
    db
      .prepare(
        `SELECT *
           FROM forum_direct_tip_webhook_events
          WHERE provider_event_ref = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(providerEventRef)
      .first<DirectTipWebhookEventRow>(),
  )

const readMoneyActionByPaymentEventId = (
  db: D1Database,
  paymentEventId: string,
): Effect.Effect<MoneyActionRow | null, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readMoneyActionByPaymentEventId', () =>
    db
      .prepare(
        `SELECT id,
                payment_event_id,
                receipt_id,
                public_projection_json
           FROM forum_money_actions
          WHERE payment_event_id = ?
          LIMIT 1`,
      )
      .bind(paymentEventId)
      .first<MoneyActionRow>(),
  )

const insertDirectTipAttempt = (
  db: D1Database,
  input: ForumDirectTipSubmitInput,
  attempt: Readonly<{
    attemptId: string
    paymentEventId: string | null
    receiptRef: string | null
    status: ForumDirectTipAttemptStatus
  }>,
  runtime: ForumPaidActionRuntime,
): Effect.Effect<void, ForumPaidActionError> =>
  d1Effect('forumPaidActions.insertDirectTipAttempt', () =>
    db
      .prepare(
        `INSERT INTO forum_direct_tip_attempts (
           id,
           idempotency_key,
           payer_actor_ref,
           recipient_actor_ref,
           target_topic_id,
           target_post_id,
           target_post_permalink,
           amount_sats,
           provider_ref,
           external_ref,
           redacted_evidence_ref,
           payment_mode,
           payment_event_status,
           status,
           receipt_ref,
           payment_event_id,
           created_at,
           updated_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        attempt.attemptId,
        input.idempotencyKey,
        input.payerActorRef,
        input.post.authorActorRef,
        input.post.topicId,
        input.post.postId,
        input.post.targetPostPermalink,
        input.amount.amount,
        input.paymentEvidence.providerRef,
        input.paymentEvidence.externalRef,
        input.paymentEvidence.redactedEvidenceRef,
        input.paymentEvidence.paymentMode,
        input.paymentEvidence.status,
        attempt.status,
        attempt.receiptRef,
        attempt.paymentEventId,
        runtime.nowIso(),
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.asVoid)

const insertDirectTipReceipt = (
  db: D1Database,
  input: ForumDirectTipSubmitInput,
  receiptId: string,
  receiptRef: string,
  runtime: ForumPaidActionRuntime,
): Effect.Effect<void, ForumPaidActionError> =>
  d1Effect('forumPaidActions.insertDirectTipReceipt', () =>
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
         VALUES (?, ?, 'post_reward', NULL, ?, ?, 'sats', ?, ?, ?, ?, ?)`,
      )
      .bind(
        receiptId,
        receiptRef,
        input.post.topicId,
        input.post.postId,
        input.amount.amount,
        input.post.authorActorRef,
        input.paymentEvidence.redactedEvidenceRef,
        JSON.stringify(
          decodeForumPublicProjection(input.post.publicProjection),
        ),
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.asVoid)

const insertDirectTipMoneyAction = (
  db: D1Database,
  input: ForumDirectTipSubmitInput,
  moneyActionId: string,
  paymentEventId: string | null,
  receiptId: string | null,
  runtime: ForumPaidActionRuntime,
): Effect.Effect<void, ForumPaidActionError> =>
  d1Effect('forumPaidActions.insertDirectTipMoneyAction', () =>
    db
      .prepare(
        `INSERT OR IGNORE INTO forum_money_actions (
           id,
           idempotency_key,
           actor_ref,
           action_kind,
           target_forum_id,
           target_topic_id,
           target_post_id,
           amount_asset,
           amount_value,
           payment_event_id,
           receipt_id,
           earning_actor_ref,
           public_projection_json,
           created_at
         )
         VALUES (?, ?, ?, 'post_reward', NULL, ?, ?, 'sats', ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        moneyActionId,
        `direct-tip:${input.idempotencyKey}`,
        input.payerActorRef,
        input.post.topicId,
        input.post.postId,
        input.amount.amount,
        paymentEventId,
        receiptId,
        input.post.authorActorRef,
        JSON.stringify(
          decodeForumPublicProjection(input.post.publicProjection),
        ),
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.asVoid)

const insertDirectTipPaymentEvent = (
  db: D1Database,
  input: ForumDirectTipSubmitInput,
  attemptId: string,
  eventId: string,
  moneyActionId: string,
  receiptRef: string | null,
  runtime: ForumPaidActionRuntime,
): Effect.Effect<void, ForumPaidActionError> =>
  d1Effect('forumPaidActions.insertDirectTipPaymentEvent', () =>
    db
      .prepare(
        `INSERT INTO forum_payment_events (
           id,
           money_action_id,
           provider_ref,
           external_ref,
           amount_asset,
           amount_value,
           redacted_evidence_ref,
           public_projection_json,
           created_at
         )
         VALUES (?, ?, ?, ?, 'sats', ?, ?, ?, ?)`,
      )
      .bind(
        eventId,
        moneyActionId,
        input.paymentEvidence.providerRef,
        input.paymentEvidence.externalRef,
        input.amount.amount,
        input.paymentEvidence.redactedEvidenceRef,
        JSON.stringify(
          directTipPaymentEventProjection({
            amount: input.amount,
            attemptId,
            eventId,
            input,
            receiptRef,
            runtime,
          }),
        ),
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.asVoid)

const insertDirectTipWebhookEvent = (
  db: D1Database,
  input: ForumDirectTipWebhookReconciliationInput,
  status: ForumDirectTipAttemptStatus,
  reconciliationResult: string,
  runtime: ForumPaidActionRuntime,
): Effect.Effect<void, ForumPaidActionError> =>
  d1Effect('forumPaidActions.insertDirectTipWebhookEvent', () =>
    db
      .prepare(
        `INSERT INTO forum_direct_tip_webhook_events (
           id,
           provider_event_ref,
           direct_tip_attempt_id,
           provider_ref,
           external_ref,
           amount_sats,
           payment_event_status,
           redacted_evidence_ref,
           event_body_digest_ref,
           signature_binding_ref,
           reconciliation_status,
           reconciliation_result,
           first_seen_at,
           last_seen_at,
           delivery_count
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)`,
      )
      .bind(
        runtime.makePaymentEventId(),
        input.providerEventRef,
        input.attemptId,
        input.paymentEvidence.providerRef,
        input.paymentEvidence.externalRef,
        input.amount.amount,
        input.paymentEvidence.status,
        input.paymentEvidence.redactedEvidenceRef,
        input.eventBodyDigestRef,
        input.signatureBindingRef,
        status,
        reconciliationResult,
        runtime.nowIso(),
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.asVoid)

const updateDirectTipWebhookEventReplay = (
  db: D1Database,
  providerEventRef: string,
  runtime: ForumPaidActionRuntime,
): Effect.Effect<void, ForumPaidActionError> =>
  d1Effect('forumPaidActions.updateDirectTipWebhookEventReplay', () =>
    db
      .prepare(
        `UPDATE forum_direct_tip_webhook_events
            SET delivery_count = delivery_count + 1,
                last_seen_at = ?
          WHERE provider_event_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(runtime.nowIso(), providerEventRef)
      .run(),
  ).pipe(Effect.asVoid)

const insertDirectTipReceiptFromAttempt = (
  db: D1Database,
  attempt: DirectTipAttemptRow,
  paymentEvidence: ForumDirectTipPaymentEvidence,
  moneyAction: MoneyActionRow,
  receiptId: string,
  receiptRef: string,
  runtime: ForumPaidActionRuntime,
): Effect.Effect<void, ForumPaidActionError> =>
  d1Effect('forumPaidActions.insertDirectTipReceiptFromAttempt', () =>
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
         VALUES (?, ?, 'post_reward', NULL, ?, ?, 'sats', ?, ?, ?, ?, ?)`,
      )
      .bind(
        receiptId,
        receiptRef,
        attempt.target_topic_id,
        attempt.target_post_id,
        attempt.amount_sats,
        attempt.recipient_actor_ref,
        paymentEvidence.redactedEvidenceRef,
        moneyAction.public_projection_json,
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.asVoid)

const updateDirectTipMoneyActionReceipt = (
  db: D1Database,
  moneyActionId: string,
  receiptId: string,
): Effect.Effect<void, ForumPaidActionError> =>
  d1Effect('forumPaidActions.updateDirectTipMoneyActionReceipt', () =>
    db
      .prepare(
        `UPDATE forum_money_actions
            SET receipt_id = ?
          WHERE id = ?`,
      )
      .bind(receiptId, moneyActionId)
      .run(),
  ).pipe(Effect.asVoid)

const updateDirectTipPaymentEventFromWebhook = (
  db: D1Database,
  attempt: DirectTipAttemptRow,
  paymentEvidence: ForumDirectTipPaymentEvidence,
  receiptRef: string | null,
  runtime: ForumPaidActionRuntime,
): Effect.Effect<void, ForumPaidActionError> =>
  Effect.gen(function* () {
    const paymentEventId = attempt.payment_event_id

    if (paymentEventId === null) {
      return
    }

    yield* d1Effect(
      'forumPaidActions.updateDirectTipPaymentEventFromWebhook',
      () =>
        db
          .prepare(
            `UPDATE forum_payment_events
                SET provider_ref = ?,
                    external_ref = ?,
                    redacted_evidence_ref = ?,
                    public_projection_json = ?
              WHERE id = ?
                AND archived_at IS NULL`,
          )
          .bind(
            paymentEvidence.providerRef,
            paymentEvidence.externalRef,
            paymentEvidence.redactedEvidenceRef,
            JSON.stringify(
              directTipWebhookPaymentEventProjection({
                amount: { amount: attempt.amount_sats, asset: 'sats' },
                attempt,
                eventId: paymentEventId,
                paymentEvidence,
                receiptRef,
                runtime,
              }),
            ),
            paymentEventId,
          )
          .run(),
    ).pipe(Effect.asVoid)
  })

const updateDirectTipAttemptFromWebhook = (
  db: D1Database,
  attemptId: string,
  paymentEvidence: ForumDirectTipPaymentEvidence,
  status: ForumDirectTipAttemptStatus,
  receiptRef: string | null,
  runtime: ForumPaidActionRuntime,
): Effect.Effect<void, ForumPaidActionError> =>
  d1Effect('forumPaidActions.updateDirectTipAttemptFromWebhook', () =>
    db
      .prepare(
        `UPDATE forum_direct_tip_attempts
            SET provider_ref = ?,
                external_ref = ?,
                redacted_evidence_ref = ?,
                payment_mode = ?,
                payment_event_status = ?,
                status = ?,
                receipt_ref = COALESCE(receipt_ref, ?),
                updated_at = ?
          WHERE id = ?
            AND archived_at IS NULL`,
      )
      .bind(
        paymentEvidence.providerRef,
        paymentEvidence.externalRef,
        paymentEvidence.redactedEvidenceRef,
        paymentEvidence.paymentMode,
        paymentEvidence.status,
        status,
        receiptRef,
        runtime.nowIso(),
        attemptId,
      )
      .run(),
  ).pipe(Effect.asVoid)

const readSettlementClaimByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<SettlementClaimRow | null, ForumPaidActionError> =>
  d1Effect('forumPaidActions.readSettlementClaimByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT id,
                idempotency_key,
                receipt_id,
                receipt_ref,
                recipient_actor_ref,
                public_projection_json,
                archived_at
           FROM forum_tip_settlement_claims
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<SettlementClaimRow>(),
  )

const insertSettlementClaim = (
  db: D1Database,
  input: ForumTipSettlementClaimInput,
  receipt: ReceiptRow,
  projection: ForumTipSettlementClaimProjection,
): Effect.Effect<void, ForumPaidActionError> =>
  d1Effect('forumPaidActions.insertSettlementClaim', () =>
    db
      .prepare(
        `INSERT OR IGNORE INTO forum_tip_settlement_claims (
           id,
           idempotency_key,
           receipt_id,
           receipt_ref,
           recipient_actor_ref,
           settlement_ref,
           settlement_evidence_refs_json,
           source_ref,
           public_projection_json,
           created_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        projection.claimId,
        input.idempotencyKey,
        receipt.id,
        receipt.receipt_ref,
        input.actorRef,
        input.settlementRef,
        JSON.stringify(input.settlementEvidenceRefs),
        input.sourceRef,
        JSON.stringify(projection),
        projection.createdAt,
      )
      .run(),
  ).pipe(Effect.asVoid)

const redemptionStatement = (
  db: D1Database,
  challenge: ChallengeRow,
  input: ForumPaidActionRedeemInput,
  receiptId: string,
  entitlementRef: string,
  runtime: ForumPaidActionRuntime,
): D1PreparedStatement =>
  db
    .prepare(
      `INSERT INTO forum_l402_redemptions (
           id,
           idempotency_key,
           challenge_id,
           actor_ref,
           proof_ref,
           entitlement_ref,
           receipt_id,
           replayed,
           public_projection_json,
           created_at
         )
         VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?)`,
    )
    .bind(
      runtime.makeRedemptionId(),
      input.idempotencyKey,
      challenge.id,
      input.actorRef,
      input.l402ProofRef,
      entitlementRef,
      receiptId,
      challenge.public_projection_json,
      runtime.nowIso(),
    )

export const previewForumPaidAction = (
  database: TreasuryDatabase,
  input: ForumPaidActionPreviewInput,
  runtime: ForumPaidActionRuntime = systemForumPaidActionRuntime,
): Effect.Effect<
  ForumPaidActionPreviewResponse,
  ForumPaidActionError | ForumPublicProjectionUnsafe
> =>
  Effect.gen(function* () {
    // KS-8.8 (#8319): D1 authority; challenge writes mirror fail-soft below.
    const db = treasuryAuthorityDb(database)
    const publicProjection = yield* validateProjection(input.publicProjection)

    if (input.nonPayableDenial !== null) {
      return deniedPreviewResponse(input.actorRef, input.nonPayableDenial)
    }

    yield* validateSpendCap(input.price, input.spendCap)
    yield* validateRewardRecipientReadiness(input)

    const immediatePolicyDenial = forumTipImmediatePreviewPolicyDenial({
      actionKind: input.actionKind,
      actorRef: input.actorRef,
      recipientActorRef: input.recipientActorRef,
    })

    if (immediatePolicyDenial !== null) {
      return deniedPreviewResponse(input.actorRef, immediatePolicyDenial)
    }

    if (input.actionKind === 'post_reward') {
      return directTipRequiredPreviewResponse(input.actorRef)
    }

    const maybeExisting = yield* readChallengeByIdempotencyKey(
      db,
      input.idempotencyKey,
    )
    const challengeId = maybeExisting?.id ?? runtime.makeChallengeId()
    const challengeExpiresAt = maybeExisting?.expires_at ?? expiresAt(runtime)

    if (maybeExisting === null) {
      const recentChallengeCount = yield* readRecentChallengeCountForActor(db, {
        actionKind: input.actionKind,
        actorRef: input.actorRef,
        sinceIso: epochMillisToIsoTimestamp(
          runtime.nowMillis() - ForumTipPreviewRateLimit.windowSeconds * 1000,
        ),
      })
      const rateLimitDenial = forumTipRateLimitPreviewPolicyDenial({
        actionKind: input.actionKind,
        recentChallengeCount,
      })

      if (rateLimitDenial !== null) {
        return deniedPreviewResponse(input.actorRef, rateLimitDenial)
      }

      const l402 = yield* issueForumMdkL402Challenge(
        input,
        challengeId,
        challengeExpiresAt,
        runtime,
      )

      yield* insertChallenge(
        db,
        input,
        challengeId,
        l402,
        JSON.stringify(publicProjection),
        challengeExpiresAt,
        runtime,
      )
      yield* Effect.promise(() =>
        mirrorTreasuryRows(database, 'forum_l402_challenges', 'id', [
          challengeId,
        ]),
      )
    }

    const storedChallenge = yield* readChallengeById(db, challengeId)

    if (storedChallenge === null) {
      return yield* new ForumPaidActionError({
        kind: 'challenge_not_found',
        reason: 'Forum L402 challenge could not be read after creation.',
      })
    }

    return decodePreviewResponse({
      challenge: challengeFromRow(storedChallenge),
      entitlementRef: null,
      paymentRequired: true,
      writeDenial: {
        actorRef: input.actorRef,
        denialKind: 'payment_required',
        denialRef: `forum_paid_action:${input.actionKind}`,
        payable: true,
        requiredPermission: null,
      },
    })
  })

const privatePaymentBindingMatches = (
  challenge: ChallengeRow,
  input: ForumPaidActionPrivatePaymentInput,
): boolean =>
  challenge.method === input.method &&
  challenge.path === input.path &&
  challenge.request_body_digest === input.requestBodyDigest &&
  routeParamsMatch(challenge.route_params_json, input.routeParams) &&
  challenge.spend_cap_asset === input.spendCap.asset &&
  challenge.spend_cap_value === input.spendCap.amount

export const readForumPaidActionPrivatePayment = (
  database: TreasuryDatabase,
  input: ForumPaidActionPrivatePaymentInput,
  runtime: ForumPaidActionRuntime = systemForumPaidActionRuntime,
): Effect.Effect<ForumPaidActionPrivatePaymentResponse, ForumPaidActionError> =>
  Effect.gen(function* () {
    const db = treasuryAuthorityDb(database)
    if (input.hostedMdkClient === undefined) {
      return yield* new ForumPaidActionError({
        kind: 'payment_provider_unconfigured',
        reason: 'detail.forum_paid_action.hosted_mdk_client_missing',
      })
    }

    if (input.signingBoundary === undefined) {
      return yield* new ForumPaidActionError({
        kind: 'payment_verification_failed',
        reason: 'Forum private L402 signer is not configured.',
      })
    }

    const challenge = yield* readChallengeById(db, input.challengeId)

    if (challenge === null) {
      return yield* new ForumPaidActionError({
        kind: 'challenge_not_found',
        reason: 'Forum L402 challenge was not found.',
      })
    }

    if (challenge.actor_ref !== input.actorRef) {
      return yield* new ForumPaidActionError({
        kind: 'actor_mismatch',
        reason:
          'Forum private L402 payment payload is available only to the challenge actor.',
      })
    }

    if (!privatePaymentBindingMatches(challenge, input)) {
      return yield* new ForumPaidActionError({
        kind: 'binding_mismatch',
        reason:
          'Forum private L402 payment payload request does not match the stored challenge binding.',
      })
    }

    yield* validateSpendCap(
      { amount: challenge.price_value, asset: challenge.price_asset },
      input.spendCap,
    )

    if (Date.parse(challenge.expires_at) <= runtime.nowMillis()) {
      return yield* new ForumPaidActionError({
        kind: 'challenge_expired',
        reason: 'Forum L402 challenge is expired. Request a fresh challenge.',
      })
    }

    const publicChallenge = challengeFromRow(challenge)
    const l402 = publicChallenge.l402

    if (l402 === null || l402.checkoutRef === null) {
      return yield* new ForumPaidActionError({
        kind: 'payment_provider_rejected',
        reason: 'detail.forum_paid_action.private_l402_payload_unavailable',
      })
    }

    const providerPayload = yield* input.hostedMdkClient
      .getPrivateL402PaymentPayload({
        checkoutRef: l402.checkoutRef,
        environment: l402.environment,
        providerRef: l402.providerRef,
        sandbox: l402.sandbox,
        siteRef: null,
      })
      .pipe(Effect.mapError(paidActionErrorFromMdkError))
    const credentialPayload = l402PayloadFromBuyerPaymentChallenge({
      challenge: buyerPaymentChallengeForRow(challenge),
      credentialRef: l402.credentialRef,
      endpointRef: l402.endpointRef,
      entitlementScopeRefs: l402.entitlementScopeRefs,
      issuedAt: runtime.nowIso(),
      paymentHashRef:
        l402.paymentHashRef ??
        `payment_hash.redacted.${cleanRefSegment(challenge.id)}`,
      replayNonceRef: l402.replayNonceRef,
    })
    const credential = yield* Effect.tryPromise({
      catch: error =>
        new ForumPaidActionError({
          kind: 'payment_verification_failed',
          reason:
            error instanceof Error
              ? error.message
              : 'Forum private L402 credential could not be minted.',
        }),
      try: () =>
        mintOpenAgentsL402Credential(
          credentialPayload,
          input.signingBoundary as OpenAgentsL402SigningBoundary,
        ),
    })

    return {
      challenge: publicChallenge,
      privatePayment: {
        bolt11: providerPayload.bolt11,
        checkoutRef: providerPayload.checkoutRef,
        credential: credential.credential,
        environment: providerPayload.environment,
        expiresAt: providerPayload.expiresAt,
        l402ProofRef: paymentProofRefForChallenge(challenge.id),
        provider: 'mdk_hosted',
        providerRef: providerPayload.providerRef,
        sandbox: providerPayload.sandbox,
      },
    }
  })

export const redeemForumPaidAction = (
  database: TreasuryDatabase,
  input: ForumPaidActionRedeemInput,
  runtime: ForumPaidActionRuntime = systemForumPaidActionRuntime,
): Effect.Effect<ForumPaidActionRedeemResponse, ForumPaidActionError> =>
  Effect.gen(function* () {
    // KS-8.8 (#8319): D1 authority; the redeem batch mirrors fail-soft below.
    const db = treasuryAuthorityDb(database)
    yield* validatePaymentProofRef(input.l402ProofRef)
    yield* validateVerifiedPaymentEvent(input.paymentEvent)

    const challenge = yield* readChallengeById(db, input.challengeId)

    if (challenge === null) {
      return yield* new ForumPaidActionError({
        kind: 'challenge_not_found',
        reason: 'Forum L402 challenge was not found.',
      })
    }

    if (Date.parse(challenge.expires_at) <= runtime.nowMillis()) {
      return yield* new ForumPaidActionError({
        kind: 'challenge_expired',
        reason: 'Forum L402 challenge is expired. Request a fresh challenge.',
      })
    }

    if (challenge.actor_ref !== input.actorRef) {
      return yield* new ForumPaidActionError({
        kind: 'actor_mismatch',
        reason: 'Forum L402 challenge actor does not match the retry actor.',
      })
    }

    const bindingMatches =
      challenge.method === input.method &&
      challenge.path === input.path &&
      challenge.recipient_actor_ref === input.recipientActorRef &&
      challenge.recipient_readiness_ref === input.recipientReadinessRef &&
      challenge.request_body_digest === input.requestBodyDigest &&
      routeParamsMatch(challenge.route_params_json, input.routeParams)

    if (!bindingMatches) {
      return yield* new ForumPaidActionError({
        kind: 'binding_mismatch',
        reason:
          'Forum L402 challenge binding does not match method, path, params, or body digest.',
      })
    }

    const existingRedemption = yield* readRedemptionByChallengeId(
      db,
      challenge.id,
    )

    if (existingRedemption !== null && existingRedemption.receipt_id !== null) {
      const existingReceipt = yield* readReceiptById(
        db,
        existingRedemption.receipt_id,
      )

      if (existingReceipt === null) {
        return yield* new ForumPaidActionError({
          kind: 'challenge_not_found',
          reason: 'Forum L402 replay receipt was not found.',
        })
      }

      return decodeRedeemResponse({
        entitlementRef: existingRedemption.entitlement_ref,
        originalReceiptRef: existingReceipt.receipt_ref,
        receiptRef: existingReceipt.receipt_ref,
        replayed: true,
      })
    }

    const verifiedPaymentEvent = input.paymentEvent ?? null

    if (verifiedPaymentEvent !== null) {
      const existingPaymentEvent = yield* readPaymentEventByProviderExternal(
        db,
        verifiedPaymentEvent.providerRef,
        verifiedPaymentEvent.externalRef,
      )

      if (existingPaymentEvent !== null) {
        return yield* new ForumPaidActionError({
          kind: 'payment_event_replayed',
          reason:
            'Forum payment event external ref was already linked to a reward receipt.',
        })
      }
    }

    const receiptId = runtime.makeReceiptId()
    const receiptRef = runtime.makeReceiptRef(challenge.id)
    const entitlementRef = runtime.makeEntitlementRef(challenge.id)
    const moneyActionId = runtime.makeMoneyActionId()
    const paymentEventId =
      verifiedPaymentEvent === null ? null : runtime.makePaymentEventId()

    yield* d1Effect('forumPaidActions.redeemWriteBatch', () =>
      db.batch([
        receiptStatement(db, challenge, input, receiptId, receiptRef, runtime),
        moneyActionStatement(
          db,
          challenge,
          input,
          moneyActionId,
          paymentEventId,
          receiptId,
          runtime,
        ),
        ...(verifiedPaymentEvent !== null && paymentEventId !== null
          ? [
              paymentEventStatement(
                db,
                challenge,
                input,
                verifiedPaymentEvent,
                paymentEventId,
                moneyActionId,
                receiptRef,
                runtime,
              ),
            ]
          : []),
        redemptionStatement(
          db,
          challenge,
          input,
          receiptId,
          entitlementRef,
          runtime,
        ),
      ]),
    )

    // KS-8.8 (#8319): mirror the money rows the batch committed —
    // read-back copies of D1, so amounts/receipt semantics port exactly.
    yield* Effect.promise(async () => {
      await mirrorTreasuryRows(database, 'forum_receipts', 'id', [receiptId])
      await mirrorTreasuryRows(database, 'forum_money_actions', 'id', [
        moneyActionId,
      ])
      if (paymentEventId !== null) {
        await mirrorTreasuryRows(database, 'forum_payment_events', 'id', [
          paymentEventId,
        ])
      }
      await mirrorTreasuryRows(
        database,
        'forum_l402_redemptions',
        'challenge_id',
        [challenge.id],
      )
    })

    return decodeRedeemResponse({
      entitlementRef,
      originalReceiptRef: null,
      receiptRef,
      replayed: false,
    })
  })

const directTipReceiptForAttempt = (
  db: D1Database,
  ledgerDb: PaymentsLedgerDb,
  attempt: DirectTipAttemptRow,
): Effect.Effect<ForumReceiptLookupResponse | null, ForumPaidActionError> =>
  attempt.receipt_ref === null
    ? Effect.succeed(null)
    : lookupForumPaidActionReceipt(db, ledgerDb, attempt.receipt_ref)

const directTipAttemptMatchesInput = (
  attempt: DirectTipAttemptRow,
  input: ForumDirectTipSubmitInput,
): boolean => {
  const targetMatches =
    attempt.payer_actor_ref === input.payerActorRef &&
    attempt.recipient_actor_ref === input.post.authorActorRef &&
    attempt.target_post_id === input.post.postId &&
    attempt.target_topic_id === input.post.topicId &&
    attempt.amount_sats === input.amount.amount

  if (!targetMatches) {
    return false
  }

  if (attempt.status === 'settled' && attempt.receipt_ref !== null) {
    return true
  }

  return (
    attempt.provider_ref === input.paymentEvidence.providerRef &&
    attempt.external_ref === input.paymentEvidence.externalRef &&
    attempt.payment_mode === input.paymentEvidence.paymentMode &&
    attempt.payment_event_status === input.paymentEvidence.status
  )
}

// #4704: legacy direct-tip attempts that sit in recovery_pending beyond
// the recovery window are ARCHIVED - status preserved (we genuinely do
// not know the wallet-side outcome; declaring failed or settled would
// both be wrong), removed from active stats and indexes. A recipient
// can still resolve a specific attempt through the settlement-claim
// path before archival. New tips ride the ladder route, where
// half-recorded attempts are structurally impossible.
export const DIRECT_TIP_RECOVERY_WINDOW_HOURS = 24

export const archiveStaleDirectTipRecoveries = async (
  database: TreasuryDatabase,
  nowIso: string,
): Promise<number> => {
  const db = treasuryAuthorityDb(database)
  const cutoffIso = epochMillisToIsoTimestamp(
    Date.parse(nowIso) - DIRECT_TIP_RECOVERY_WINDOW_HOURS * 3_600_000,
  )

  // KS-8.8 (#8319): capture the affected attempt ids BEFORE the archive
  // update so the fail-soft Postgres mirror can read back exactly the rows
  // this cron touched (bounded page; the stale set is small by design).
  const staleIds = (
    (
      await db
        .prepare(
          `SELECT id FROM forum_direct_tip_attempts
           WHERE status = 'recovery_pending'
             AND archived_at IS NULL
             AND updated_at < ?
           LIMIT 200`,
        )
        .bind(cutoffIso)
        .all<{ id: string }>()
    ).results ?? []
  ).map(row => row.id)

  const result = await db
    .prepare(
      `UPDATE forum_direct_tip_attempts
       SET archived_at = ?, updated_at = ?
       WHERE status = 'recovery_pending'
         AND archived_at IS NULL
         AND updated_at < ?`,
    )
    .bind(nowIso, nowIso, cutoffIso)
    .run()

  await mirrorTreasuryRows(
    database,
    'forum_direct_tip_attempts',
    'id',
    staleIds,
  )

  return result.meta?.changes ?? 0
}

export const submitForumDirectTip = (
  database: TreasuryDatabase,
  // CFG-4 (#8519): only the tip-ladder receipt lookup reads this handle;
  // the direct-tip tables themselves stay on the D1 treasury authority.
  ledgerDb: PaymentsLedgerDb,
  input: ForumDirectTipSubmitInput,
  runtime: ForumPaidActionRuntime = systemForumPaidActionRuntime,
): Effect.Effect<
  ForumDirectTipResponse,
  ForumPaidActionError | ForumPublicProjectionUnsafe
> =>
  Effect.gen(function* () {
    // KS-8.8 (#8319): D1 authority; settled tip rows mirror fail-soft below.
    const db = treasuryAuthorityDb(database)
    yield* validateDirectTipSubmitInput(input)

    const existingByIdempotency = yield* readDirectTipAttemptByIdempotencyKey(
      db,
      input.idempotencyKey,
    )

    if (existingByIdempotency !== null) {
      if (!directTipAttemptMatchesInput(existingByIdempotency, input)) {
        return yield* new ForumPaidActionError({
          kind: 'binding_mismatch',
          reason:
            'Idempotency-Key already belongs to a different Forum direct tip.',
        })
      }

      const receipt = yield* directTipReceiptForAttempt(
        db,
        ledgerDb,
        existingByIdempotency,
      )

      return directTipResponse(existingByIdempotency, receipt, true)
    }

    const existingByProvider = yield* readDirectTipAttemptByProviderExternal(
      db,
      input.paymentEvidence.providerRef,
      input.paymentEvidence.externalRef,
    )

    if (existingByProvider !== null) {
      return yield* new ForumPaidActionError({
        kind: 'payment_event_replayed',
        reason:
          'Forum direct tip provider/external payment ref already belongs to another attempt.',
      })
    }

    const existingPaymentEvent = yield* readPaymentEventByProviderExternal(
      db,
      input.paymentEvidence.providerRef,
      input.paymentEvidence.externalRef,
    )

    if (existingPaymentEvent !== null) {
      return yield* new ForumPaidActionError({
        kind: 'payment_event_replayed',
        reason:
          'Forum payment event external ref was already linked to another receipt.',
      })
    }

    const attemptId = runtime.makeChallengeId()
    const attemptStatus = directTipStatusForPaymentEvent(
      input.paymentEvidence.status,
    )
    const paymentEventId = runtime.makePaymentEventId()
    const moneyActionId = runtime.makeMoneyActionId()
    const receiptId =
      attemptStatus === 'settled' ? runtime.makeReceiptId() : null
    const receiptRef =
      receiptId === null ? null : `receipt.forum.direct_tip.${attemptId}`

    if (receiptId !== null && receiptRef !== null) {
      yield* insertDirectTipReceipt(db, input, receiptId, receiptRef, runtime)
    }

    yield* insertDirectTipMoneyAction(
      db,
      input,
      moneyActionId,
      paymentEventId,
      receiptId,
      runtime,
    )
    yield* insertDirectTipPaymentEvent(
      db,
      input,
      attemptId,
      paymentEventId,
      moneyActionId,
      receiptRef,
      runtime,
    )
    yield* insertDirectTipAttempt(
      db,
      input,
      {
        attemptId,
        paymentEventId,
        receiptRef,
        status: attemptStatus,
      },
      runtime,
    )

    const storedAttempt = yield* readDirectTipAttemptById(db, attemptId)

    if (storedAttempt === null) {
      return yield* new ForumPaidActionError({
        kind: 'receipt_not_found',
        reason: 'Forum direct tip attempt was not found after insert.',
      })
    }

    // KS-8.8 (#8319): mirror the tip rows just written (read-back copies).
    yield* Effect.promise(async () => {
      if (receiptId !== null) {
        await mirrorTreasuryRows(database, 'forum_receipts', 'id', [receiptId])
      }
      await mirrorTreasuryRows(database, 'forum_money_actions', 'id', [
        moneyActionId,
      ])
      await mirrorTreasuryRows(database, 'forum_payment_events', 'id', [
        paymentEventId,
      ])
      await mirrorTreasuryRows(database, 'forum_direct_tip_attempts', 'id', [
        attemptId,
      ])
    })

    const receipt = yield* directTipReceiptForAttempt(db, ledgerDb, storedAttempt)

    return directTipResponse(storedAttempt, receipt, false)
  })

export const lookupForumDirectTip = (
  database: TreasuryDatabase,
  // CFG-4 (#8519): tip-ladder receipt rows live on the Postgres ledger.
  ledgerDb: PaymentsLedgerDb,
  attemptId: string,
): Effect.Effect<ForumDirectTipResponse | null, ForumPaidActionError> =>
  Effect.gen(function* () {
    const db = treasuryAuthorityDb(database)
    const attempt = yield* readDirectTipAttemptById(db, attemptId)

    if (attempt === null) {
      return null
    }

    const receipt = yield* directTipReceiptForAttempt(db, ledgerDb, attempt)

    return directTipResponse(attempt, receipt, true)
  })

export const reconcileForumDirectTipWebhook = (
  database: TreasuryDatabase,
  // CFG-4 (#8519): tip-ladder receipt rows live on the Postgres ledger.
  ledgerDb: PaymentsLedgerDb,
  input: ForumDirectTipWebhookReconciliationInput,
  runtime: ForumPaidActionRuntime = systemForumPaidActionRuntime,
): Effect.Effect<ForumDirectTipWebhookReconciliation, ForumPaidActionError> =>
  Effect.gen(function* () {
    // KS-8.8 (#8319): D1 authority; reconciled rows mirror fail-soft below.
    const db = treasuryAuthorityDb(database)
    yield* validatePaymentEventRefs(input.paymentEvidence)
    yield* validatePaymentEventRef('providerEventRef', input.providerEventRef)
    yield* validatePaymentEventRef(
      'eventBodyDigestRef',
      input.eventBodyDigestRef,
    )
    yield* validatePaymentEventRef(
      'signatureBindingRef',
      input.signatureBindingRef,
    )

    if (input.amount.asset !== 'sats' || input.amount.amount <= 0) {
      return yield* new ForumPaidActionError({
        kind: 'over_spend_cap',
        reason:
          'Forum direct-tip webhooks must reconcile positive sats amounts.',
      })
    }

    const attempt = yield* readDirectTipAttemptById(db, input.attemptId)

    if (attempt === null) {
      return yield* new ForumPaidActionError({
        kind: 'challenge_not_found',
        reason: 'Forum direct tip attempt was not found for MDK webhook.',
      })
    }

    if (attempt.amount_sats !== input.amount.amount) {
      return yield* new ForumPaidActionError({
        kind: 'binding_mismatch',
        reason: 'Forum direct tip webhook amount does not match the attempt.',
      })
    }

    const existingWebhook = yield* readDirectTipWebhookEventByProviderEventRef(
      db,
      input.providerEventRef,
    )

    if (existingWebhook !== null) {
      if (
        existingWebhook.direct_tip_attempt_id !== input.attemptId ||
        existingWebhook.amount_sats !== input.amount.amount ||
        existingWebhook.provider_ref !== input.paymentEvidence.providerRef ||
        existingWebhook.external_ref !== input.paymentEvidence.externalRef
      ) {
        return yield* new ForumPaidActionError({
          kind: 'payment_event_replayed',
          reason:
            'Forum MDK webhook provider event ref already belongs to another direct tip.',
        })
      }

      yield* updateDirectTipWebhookEventReplay(
        db,
        input.providerEventRef,
        runtime,
      )
      yield* Effect.promise(() =>
        mirrorTreasuryRows(
          database,
          'forum_direct_tip_webhook_events',
          'provider_event_ref',
          [input.providerEventRef],
        ),
      )

      const refreshedAttempt =
        (yield* readDirectTipAttemptById(db, input.attemptId)) ?? attempt
      const receipt = yield* directTipReceiptForAttempt(db, ledgerDb, refreshedAttempt)

      return decodeDirectTipWebhookReconciliation({
        amount: input.amount,
        attemptId: input.attemptId,
        eventBodyDigestRef: existingWebhook.event_body_digest_ref,
        idempotent: true,
        paymentEvidence: {
          externalRef: existingWebhook.external_ref,
          paymentMode: refreshedAttempt.payment_mode,
          providerRef: existingWebhook.provider_ref,
          redactedEvidenceRef: existingWebhook.redacted_evidence_ref,
          status: existingWebhook.payment_event_status,
        },
        receipt,
        reconciliationRef: existingWebhook.id,
        signatureBindingRef: existingWebhook.signature_binding_ref,
        status: refreshedAttempt.status,
      })
    }

    const existingPaymentEvent = yield* readPaymentEventByProviderExternal(
      db,
      input.paymentEvidence.providerRef,
      input.paymentEvidence.externalRef,
    )

    if (
      existingPaymentEvent !== null &&
      existingPaymentEvent.id !== attempt.payment_event_id
    ) {
      return yield* new ForumPaidActionError({
        kind: 'payment_event_replayed',
        reason:
          'Forum direct-tip webhook payment ref already belongs to another payment event.',
      })
    }

    const status = directTipStatusForPaymentEvent(input.paymentEvidence.status)
    let receiptRef = attempt.receipt_ref

    if (status === 'settled' && receiptRef === null) {
      if (attempt.payment_event_id === null) {
        return yield* new ForumPaidActionError({
          kind: 'receipt_not_found',
          reason:
            'Forum direct-tip webhook could not locate the original payment event.',
        })
      }

      const moneyAction = yield* readMoneyActionByPaymentEventId(
        db,
        attempt.payment_event_id,
      )

      if (moneyAction === null) {
        return yield* new ForumPaidActionError({
          kind: 'receipt_not_found',
          reason:
            'Forum direct-tip webhook could not locate the original money action.',
        })
      }

      const receiptId = runtime.makeReceiptId()
      receiptRef = `receipt.forum.direct_tip.${attempt.id}`
      yield* insertDirectTipReceiptFromAttempt(
        db,
        attempt,
        input.paymentEvidence,
        moneyAction,
        receiptId,
        receiptRef,
        runtime,
      )
      yield* updateDirectTipMoneyActionReceipt(db, moneyAction.id, receiptId)
    }

    yield* updateDirectTipPaymentEventFromWebhook(
      db,
      attempt,
      input.paymentEvidence,
      receiptRef,
      runtime,
    )
    yield* updateDirectTipAttemptFromWebhook(
      db,
      attempt.id,
      input.paymentEvidence,
      status,
      receiptRef,
      runtime,
    )
    yield* insertDirectTipWebhookEvent(
      db,
      input,
      status,
      status === 'settled' ? 'receipt_settled' : 'attempt_recorded',
      runtime,
    )

    // KS-8.8 (#8319): mirror every row this reconciliation touched.
    yield* Effect.promise(async () => {
      if (receiptRef !== null) {
        await mirrorTreasuryRows(database, 'forum_receipts', 'receipt_ref', [
          receiptRef,
        ])
      }
      if (attempt.payment_event_id !== null) {
        await mirrorTreasuryRows(database, 'forum_payment_events', 'id', [
          attempt.payment_event_id,
        ])
        await mirrorTreasuryRows(
          database,
          'forum_money_actions',
          'payment_event_id',
          [attempt.payment_event_id],
        )
      }
      await mirrorTreasuryRows(database, 'forum_direct_tip_attempts', 'id', [
        attempt.id,
      ])
      await mirrorTreasuryRows(
        database,
        'forum_direct_tip_webhook_events',
        'provider_event_ref',
        [input.providerEventRef],
      )
    })

    const storedAttempt =
      (yield* readDirectTipAttemptById(db, attempt.id)) ?? attempt
    const receipt = yield* directTipReceiptForAttempt(db, ledgerDb, storedAttempt)
    const webhook = yield* readDirectTipWebhookEventByProviderEventRef(
      db,
      input.providerEventRef,
    )

    return decodeDirectTipWebhookReconciliation({
      amount: input.amount,
      attemptId: input.attemptId,
      eventBodyDigestRef: input.eventBodyDigestRef,
      idempotent: false,
      paymentEvidence: input.paymentEvidence,
      receipt,
      reconciliationRef: webhook?.id ?? input.providerEventRef,
      signatureBindingRef: input.signatureBindingRef,
      status,
    })
  })

const settlementClaimResponse = (
  receipt: ForumReceiptLookupResponse,
  idempotent: boolean,
): Effect.Effect<ForumTipSettlementClaimResponse, ForumPaidActionError> =>
  Effect.gen(function* () {
    if (receipt.settlementClaim === null) {
      return yield* new ForumPaidActionError({
        kind: 'settlement_claim_unavailable',
        reason: 'Forum tip settlement claim was not persisted.',
      })
    }

    return decodeSettlementClaimResponse({
      idempotent,
      receipt,
      settlementClaim: receipt.settlementClaim,
    })
  })

export const claimForumTipSettlement = (
  database: TreasuryDatabase,
  // CFG-4 (#8519): tip-ladder receipt rows live on the Postgres ledger.
  ledgerDb: PaymentsLedgerDb,
  input: ForumTipSettlementClaimInput,
  runtime: ForumPaidActionRuntime = systemForumPaidActionRuntime,
): Effect.Effect<ForumTipSettlementClaimResponse, ForumPaidActionError> =>
  Effect.gen(function* () {
    // KS-8.8 (#8319): D1 authority; the claim row mirrors fail-soft below.
    const db = treasuryAuthorityDb(database)
    yield* validateSettlementRefs(input)

    const existingByIdempotency = yield* readSettlementClaimByIdempotencyKey(
      db,
      input.idempotencyKey,
    )

    if (existingByIdempotency !== null) {
      if (existingByIdempotency.receipt_ref !== input.receiptRef) {
        return yield* new ForumPaidActionError({
          kind: 'binding_mismatch',
          reason:
            'Idempotency-Key already belongs to a different Forum settlement claim.',
        })
      }

      const existingReceipt = yield* lookupForumPaidActionReceipt(
        db,
        ledgerDb,
        input.receiptRef,
      )

      if (existingReceipt === null) {
        return yield* new ForumPaidActionError({
          kind: 'receipt_not_found',
          reason: 'Forum receipt for settlement claim was not found.',
        })
      }

      return yield* settlementClaimResponse(existingReceipt, true)
    }

    const receiptRow = yield* readReceiptLookupRowByRef(db, input.receiptRef)

    if (receiptRow === null) {
      return yield* new ForumPaidActionError({
        kind: 'receipt_not_found',
        reason: 'Forum receipt for settlement claim was not found.',
      })
    }

    if (receiptRow.recipient_actor_ref !== input.actorRef) {
      return yield* new ForumPaidActionError({
        kind: 'recipient_actor_mismatch',
        reason:
          'Forum settlement claim actor must match the receipt recipient actor.',
      })
    }

    const paymentEvent = paymentEventProjectionFromRow(receiptRow)

    if (paymentEvent?.status !== 'confirmed') {
      return yield* new ForumPaidActionError({
        kind: 'payment_verification_failed',
        reason:
          'Forum settlement claims require confirmed payer payment evidence on the receipt.',
      })
    }

    const existingClaim = settlementClaimProjectionFromRow(receiptRow)

    if (existingClaim !== null) {
      return yield* settlementClaimResponse(
        receiptLookupFromRow(receiptRow),
        true,
      )
    }

    const projection = settlementClaimProjection({
      claimId: runtime.makeSettlementClaimId?.() ?? randomUuid(),
      createdAt: runtime.nowIso(),
      input,
    })

    yield* insertSettlementClaim(db, input, receiptRow, projection)
    yield* Effect.promise(() =>
      mirrorTreasuryRows(
        database,
        'forum_tip_settlement_claims',
        'idempotency_key',
        [input.idempotencyKey],
      ),
    )

    const updatedReceipt = yield* lookupForumPaidActionReceipt(
      db,
      ledgerDb,
      input.receiptRef,
    )

    if (updatedReceipt === null) {
      return yield* new ForumPaidActionError({
        kind: 'receipt_not_found',
        reason:
          'Forum receipt for settlement claim was not found after insert.',
      })
    }

    return yield* settlementClaimResponse(
      updatedReceipt,
      updatedReceipt.settlementClaim?.claimId !== projection.claimId,
    )
  })

export const lookupForumPaidActionChallenge = (
  database: TreasuryDatabase,
  challengeId: string,
): Effect.Effect<ForumL402Challenge | null, ForumPaidActionError> =>
  readChallengeById(treasuryAuthorityDb(database), challengeId).pipe(
    Effect.map(row => (row === null ? null : challengeFromRow(row))),
  )

export const lookupForumPaidActionReceipt = (
  database: TreasuryDatabase,
  // CFG-4 (#8519): tip-ladder receipt rows live on the Postgres ledger.
  ledgerDb: PaymentsLedgerDb,
  receiptRef: string,
): Effect.Effect<ForumReceiptLookupResponse | null, ForumPaidActionError> =>
  Effect.gen(function* () {
    const db = treasuryAuthorityDb(database)
    const receiptRow = yield* readReceiptLookupRowByRef(db, receiptRef)

    if (receiptRow !== null) {
      return receiptLookupFromRow(receiptRow)
    }

    const ladderRow = yield* readTipLadderReceiptLookupRowByRef(
      ledgerDb,
      receiptRef,
    )

    return ladderRow === null ? null : tipLadderReceiptLookupFromRow(ladderRow)
  })
