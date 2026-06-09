import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const WebhookSubscriptionStatus = S.Literals([
  'active',
  'paused',
  'pending_review',
  'revoked',
])
export type WebhookSubscriptionStatus =
  typeof WebhookSubscriptionStatus.Type

export const WebhookEventFamily = S.Literals([
  'forum_payment_receipt',
  'package_review',
  'payment_reconciliation',
  'program_run',
  'public_claim',
  'receipt',
  'site_revision',
  'site_version',
  'workroom',
])
export type WebhookEventFamily = typeof WebhookEventFamily.Type

export const WebhookAuthMode = S.Literals([
  'operator_admin',
  'owner_grant',
  'registered_agent_grant',
  'system',
  'team_scope',
])
export type WebhookAuthMode = typeof WebhookAuthMode.Type

export const WebhookRetryPolicy = S.Literals([
  'exponential',
  'linear',
  'none',
  'operator_review',
])
export type WebhookRetryPolicy = typeof WebhookRetryPolicy.Type

export const WebhookDeliveryState = S.Literals([
  'dead_lettered',
  'delivered',
  'delivering',
  'failed',
  'paused',
  'pending',
  'queued',
  'retry_scheduled',
])
export type WebhookDeliveryState = typeof WebhookDeliveryState.Type

export const WebhookFailureClass = S.Literals([
  'auth_failed',
  'endpoint_unavailable',
  'none',
  'payload_rejected',
  'policy_denied',
  'rate_limited',
  'redaction_failed',
  'secret_missing',
  'timeout',
  'unknown',
])
export type WebhookFailureClass = typeof WebhookFailureClass.Type

export const WebhookEventStatus = S.Literals([
  'blocked',
  'held',
  'ready',
])
export type WebhookEventStatus = typeof WebhookEventStatus.Type

export const ProgramRunReceiptWebhookLifecycleState = S.Literals([
  'blocked',
  'delivery_attempt_recorded',
  'delivery_prepared',
  'event_selection_recorded',
  'receipt_recorded',
  'registration_recorded',
  'replay_window_recorded',
  'retry_scheduled',
  'revoked',
])
export type ProgramRunReceiptWebhookLifecycleState =
  typeof ProgramRunReceiptWebhookLifecycleState.Type

export const ProgramRunReceiptWebhookAuthorityBoundary =
  S.Literals(['contract_only'])
export type ProgramRunReceiptWebhookAuthorityBoundary =
  typeof ProgramRunReceiptWebhookAuthorityBoundary.Type

export class ProgramRunReceiptWebhookAuthority extends S.Class<ProgramRunReceiptWebhookAuthority>(
  'ProgramRunReceiptWebhookAuthority',
)({
  authEscalationDenied: S.Boolean,
  authorityBoundary: ProgramRunReceiptWebhookAuthorityBoundary,
  noDeliveryQueueEnqueue: S.Boolean,
  noExternalWebhookCall: S.Boolean,
  noPaymentMutation: S.Boolean,
  noProgramRunMutation: S.Boolean,
  noReceiptMutation: S.Boolean,
  noSecretMaterial: S.Boolean,
}) {}

export class WebhookSubscriptionRecord extends S.Class<WebhookSubscriptionRecord>(
  'WebhookSubscriptionRecord',
)({
  authMode: WebhookAuthMode,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  endpointRef: S.String,
  eventFamilies: S.Array(WebhookEventFamily),
  eventSourceRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  maxAttempts: S.Number,
  ownerRef: S.String,
  redactionAudience: S.Literals(['public', 'customer', 'team', 'operator']),
  retryPolicy: WebhookRetryPolicy,
  scopeRefs: S.Array(S.String),
  secretBindingRef: S.NullOr(S.String),
  status: WebhookSubscriptionStatus,
  subscriptionRef: S.String,
  updatedAtIso: S.String,
}) {}

export class WebhookEventRecord extends S.Class<WebhookEventRecord>(
  'WebhookEventRecord',
)({
  caveatRefs: S.Array(S.String),
  eventRef: S.String,
  family: WebhookEventFamily,
  id: S.String,
  idempotencyKey: S.String,
  occurredAtIso: S.String,
  payloadDigestRef: S.String,
  payloadSchemaRef: S.String,
  receiptRefs: S.Array(S.String),
  redactionAudience: S.Literals(['public', 'customer', 'team', 'operator']),
  replayKey: S.String,
  sourceAuthorityRefs: S.Array(S.String),
  sourceRef: S.String,
  status: WebhookEventStatus,
  subjectRef: S.String,
}) {}

export class WebhookDeliveryRecord extends S.Class<WebhookDeliveryRecord>(
  'WebhookDeliveryRecord',
)({
  attempt: S.Number,
  createdAtIso: S.String,
  deliveredAtIso: S.NullOr(S.String),
  deliveryReceiptRefs: S.Array(S.String),
  endpointRef: S.String,
  eventRef: S.String,
  failureClass: WebhookFailureClass,
  failureSummaryRef: S.NullOr(S.String),
  id: S.String,
  idempotencyKey: S.String,
  maxAttempts: S.Number,
  nextAttemptAtIso: S.NullOr(S.String),
  payloadDigestRef: S.String,
  redactionAudience: S.Literals(['public', 'customer', 'team', 'operator']),
  replayKey: S.String,
  retryPolicy: WebhookRetryPolicy,
  state: WebhookDeliveryState,
  subscriptionRef: S.String,
  updatedAtIso: S.String,
}) {}

export class ProgramRunReceiptWebhookSubscriptionContract extends S.Class<ProgramRunReceiptWebhookSubscriptionContract>(
  'ProgramRunReceiptWebhookSubscriptionContract',
)({
  authority: ProgramRunReceiptWebhookAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  deliveryAttemptRefs: S.Array(S.String),
  deliveryPreparationRefs: S.Array(S.String),
  endpointRefs: S.Array(S.String),
  eventFamilies: S.Array(WebhookEventFamily),
  eventTopicRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  idempotencyRefs: S.Array(S.String),
  lastEventRef: S.NullOr(S.String),
  lifecycleState: ProgramRunReceiptWebhookLifecycleState,
  maxAttempts: S.Number,
  operatorDiagnosticRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  redactionAudience: S.Literals(['public', 'customer', 'team', 'operator']),
  redactionPolicyRefs: S.Array(S.String),
  replayWindowRefs: S.Array(S.String),
  retryPolicy: WebhookRetryPolicy,
  retryStateRefs: S.Array(S.String),
  revocationRefs: S.Array(S.String),
  scopedAuthRefs: S.Array(S.String),
  status: WebhookSubscriptionStatus,
  subscriberRefs: S.Array(S.String),
  subscriptionRef: S.String,
  updatedAtIso: S.String,
}) {}

export class WebhookSubscriptionProjection extends S.Class<WebhookSubscriptionProjection>(
  'WebhookSubscriptionProjection',
)({
  authMode: WebhookAuthMode,
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  caveatRefs: S.Array(S.String),
  endpointRef: S.String,
  eventFamilies: S.Array(WebhookEventFamily),
  eventSourceRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  id: S.String,
  maxAttempts: S.Number,
  ownerRef: S.String,
  redactionAudience: S.Literals(['public', 'customer', 'team', 'operator']),
  retryPolicy: WebhookRetryPolicy,
  scopeRefs: S.Array(S.String),
  secretBindingRef: S.NullOr(S.String),
  status: WebhookSubscriptionStatus,
  statusLabel: S.String,
  subscriptionRef: S.String,
  updatedAtDisplay: S.String,
}) {}

export class WebhookEventProjection extends S.Class<WebhookEventProjection>(
  'WebhookEventProjection',
)({
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  caveatRefs: S.Array(S.String),
  eventRef: S.String,
  family: WebhookEventFamily,
  id: S.String,
  idempotencyKey: S.String,
  occurredAtDisplay: S.String,
  payloadDigestRef: S.String,
  payloadSchemaRef: S.String,
  receiptRefs: S.Array(S.String),
  redactionAudience: S.Literals(['public', 'customer', 'team', 'operator']),
  replayKey: S.String,
  sourceAuthorityRefs: S.Array(S.String),
  sourceRef: S.String,
  status: WebhookEventStatus,
  statusLabel: S.String,
  subjectRef: S.String,
}) {}

export class WebhookDeliveryProjection extends S.Class<WebhookDeliveryProjection>(
  'WebhookDeliveryProjection',
)({
  attempt: S.Number,
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  canRetry: S.Boolean,
  createdAtDisplay: S.String,
  deliveredAtDisplay: S.NullOr(S.String),
  deliveryReceiptRefs: S.Array(S.String),
  endpointRef: S.String,
  eventRef: S.String,
  failureClass: WebhookFailureClass,
  failureClassLabel: S.String,
  failureSummaryRef: S.NullOr(S.String),
  id: S.String,
  idempotencyKey: S.String,
  maxAttempts: S.Number,
  nextAttemptAtDisplay: S.NullOr(S.String),
  payloadDigestRef: S.String,
  redactionAudience: S.Literals(['public', 'customer', 'team', 'operator']),
  replayKey: S.String,
  retryPolicy: WebhookRetryPolicy,
  state: WebhookDeliveryState,
  stateLabel: S.String,
  subscriptionRef: S.String,
  updatedAtDisplay: S.String,
}) {}

export class ProgramRunReceiptWebhookProjection extends S.Class<ProgramRunReceiptWebhookProjection>(
  'ProgramRunReceiptWebhookProjection',
)({
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  authority: ProgramRunReceiptWebhookAuthority,
  authEscalationAllowed: S.Boolean,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  deliveryAttemptRecorded: S.Boolean,
  deliveryAttemptRefs: S.Array(S.String),
  deliveryPreparationRecorded: S.Boolean,
  deliveryPreparationRefs: S.Array(S.String),
  deliveryQueueEnqueueAllowed: S.Boolean,
  endpointRefs: S.Array(S.String),
  eventFamilies: S.Array(WebhookEventFamily),
  eventSelectionRecorded: S.Boolean,
  eventTopicRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  externalWebhookCallAllowed: S.Boolean,
  id: S.String,
  idempotencyRefs: S.Array(S.String),
  lastEventRef: S.NullOr(S.String),
  lifecycleLabel: S.String,
  lifecycleState: ProgramRunReceiptWebhookLifecycleState,
  maxAttempts: S.Number,
  operatorDiagnosticRefs: S.Array(S.String),
  paymentMutationAllowed: S.Boolean,
  programRunMutationAllowed: S.Boolean,
  receiptRecorded: S.Boolean,
  receiptRefs: S.Array(S.String),
  receiptMutationAllowed: S.Boolean,
  redactionAudience: S.Literals(['public', 'customer', 'team', 'operator']),
  redactionPolicyRefs: S.Array(S.String),
  replayWindowRecorded: S.Boolean,
  replayWindowRefs: S.Array(S.String),
  retryPolicy: WebhookRetryPolicy,
  retryScheduled: S.Boolean,
  retryStateRefs: S.Array(S.String),
  revoked: S.Boolean,
  revocationRefs: S.Array(S.String),
  scopedAuthRefs: S.Array(S.String),
  secretMaterialAllowed: S.Boolean,
  status: WebhookSubscriptionStatus,
  statusLabel: S.String,
  subscriberRefs: S.Array(S.String),
  subscriptionRef: S.String,
  subscriptionRegistrationRecorded: S.Boolean,
  updatedAtDisplay: S.String,
}) {}

export class WebhookSubscriptionUnsafe extends S.TaggedErrorClass<WebhookSubscriptionUnsafe>()(
  'WebhookSubscriptionUnsafe',
  {
    reason: S.String,
  },
) {}

const subscriptionStatusLabel: Record<WebhookSubscriptionStatus, string> = {
  active: 'Active',
  paused: 'Paused',
  pending_review: 'Pending review',
  revoked: 'Revoked',
}

const eventStatusLabel: Record<WebhookEventStatus, string> = {
  blocked: 'Blocked',
  held: 'Held',
  ready: 'Ready',
}

const programRunReceiptWebhookLifecycleLabel:
  Record<ProgramRunReceiptWebhookLifecycleState, string> = {
    blocked: 'Blocked',
    delivery_attempt_recorded: 'Delivery attempt recorded',
    delivery_prepared: 'Delivery prepared',
    event_selection_recorded: 'Event selection recorded',
    receipt_recorded: 'Receipt recorded',
    registration_recorded: 'Registration recorded',
    replay_window_recorded: 'Replay window recorded',
    retry_scheduled: 'Retry scheduled',
    revoked: 'Revoked',
  }

const deliveryStateLabel: Record<WebhookDeliveryState, string> = {
  dead_lettered: 'Dead lettered',
  delivered: 'Delivered',
  delivering: 'Delivering',
  failed: 'Failed',
  paused: 'Paused',
  pending: 'Pending',
  queued: 'Queued',
  retry_scheduled: 'Retry scheduled',
}

const failureClassLabel: Record<WebhookFailureClass, string> = {
  auth_failed: 'Auth failed',
  endpoint_unavailable: 'Endpoint unavailable',
  none: 'None',
  payload_rejected: 'Payload rejected',
  policy_denied: 'Policy denied',
  rate_limited: 'Rate limited',
  redaction_failed: 'Redaction failed',
  secret_missing: 'Secret missing',
  timeout: 'Timeout',
  unknown: 'Unknown',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeWebhookRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|callback[_-]?token|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|response|token)|raw[_-]?(body|email|invoice|payment|payload|prompt|provider|response|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret[_-]?(key|ref|token|value)|sk-[a-z0-9]|source[_-]?archive|token|wallet|webhook[_-]?secret)/i
const isoTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(auth\.private|diagnostic\.private|endpoint\.private|owner\.private|scope\.private|source\.private|subject\.private|subscriber\.private)/i
const customerUnsafeRefPattern =
  /(auth\.private|diagnostic\.private|endpoint\.private|owner\.private|scope\.private|source\.private|subject\.private|subscriber\.private)/i
const teamUnsafeRefPattern =
  /(auth\.private|diagnostic\.private|endpoint\.private|owner\.private|source\.private|subject\.private|subscriber\.private)/i
const nonRetriableFailures: ReadonlySet<WebhookFailureClass> = new Set([
  'none',
  'policy_denied',
  'redaction_failed',
  'secret_missing',
])
const programRunReceiptWebhookAllowedFamilies:
  ReadonlySet<WebhookEventFamily> = new Set(['program_run', 'receipt'])

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
    unsafeWebhookRefPattern.test(ref) ||
    isoTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new WebhookSubscriptionUnsafe({
      reason: `${label} contains webhook secrets, raw payloads, raw provider responses, tokens, customer private data, wallet/payment material, private repo refs, raw runner logs, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: BlueprintMissionBriefingAudience,
): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: BlueprintMissionBriefingAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const safeRefForAudience = (
  label: string,
  ref: string,
  audience: BlueprintMissionBriefingAudience,
): string =>
  safeRefsForAudience(label, [ref], audience)[0] ??
  `${label.replaceAll(' ', '_')}.redacted`

const maybeSafeRefForAudience = (
  label: string,
  ref: string | null,
  audience: BlueprintMissionBriefingAudience,
): string | null =>
  ref === null ? null : safeRefsForAudience(label, [ref], audience)[0] ?? null

const assertSubscriptionSafe = (
  subscription: WebhookSubscriptionRecord,
): void => {
  assertSafeRefs('webhook subscription identity refs', [
    subscription.id,
    subscription.subscriptionRef,
    subscription.ownerRef,
    subscription.endpointRef,
  ])
  assertSafeRefs(
    'webhook subscription secret binding refs',
    subscription.secretBindingRef === null ? [] : [subscription.secretBindingRef],
  )
  assertSafeRefs('webhook subscription event source refs', subscription.eventSourceRefs)
  assertSafeRefs('webhook subscription scope refs', subscription.scopeRefs)
  assertSafeRefs('webhook subscription caveat refs', subscription.caveatRefs)
  assertSafeRefs('webhook subscription evidence refs', subscription.evidenceRefs)

  if (subscription.eventFamilies.length === 0) {
    throw new WebhookSubscriptionUnsafe({
      reason: 'Webhook subscriptions require at least one event family.',
    })
  }

  if (!Number.isInteger(subscription.maxAttempts) || subscription.maxAttempts < 0) {
    throw new WebhookSubscriptionUnsafe({
      reason: 'Webhook max attempts must be a non-negative integer.',
    })
  }
}

const assertEventSafe = (event: WebhookEventRecord): void => {
  assertSafeRefs('webhook event identity refs', [
    event.id,
    event.eventRef,
    event.sourceRef,
    event.subjectRef,
    event.payloadDigestRef,
    event.payloadSchemaRef,
    event.replayKey,
    event.idempotencyKey,
  ])
  assertSafeRefs('webhook event source authority refs', event.sourceAuthorityRefs)
  assertSafeRefs('webhook event receipt refs', event.receiptRefs)
  assertSafeRefs('webhook event caveat refs', event.caveatRefs)
}

const assertDeliverySafe = (delivery: WebhookDeliveryRecord): void => {
  assertSafeRefs('webhook delivery identity refs', [
    delivery.id,
    delivery.subscriptionRef,
    delivery.eventRef,
    delivery.endpointRef,
    delivery.payloadDigestRef,
    delivery.replayKey,
    delivery.idempotencyKey,
  ])
  assertSafeRefs(
    'webhook delivery failure summary refs',
    delivery.failureSummaryRef === null ? [] : [delivery.failureSummaryRef],
  )
  assertSafeRefs('webhook delivery receipt refs', delivery.deliveryReceiptRefs)

  if (!Number.isInteger(delivery.attempt) || delivery.attempt < 0) {
    throw new WebhookSubscriptionUnsafe({
      reason: 'Webhook delivery attempt must be a non-negative integer.',
    })
  }

  if (!Number.isInteger(delivery.maxAttempts) || delivery.maxAttempts < 0) {
    throw new WebhookSubscriptionUnsafe({
      reason: 'Webhook delivery max attempts must be a non-negative integer.',
    })
  }
}

export const programRunReceiptWebhookAuthorityIsContractOnly = (
  authority: ProgramRunReceiptWebhookAuthority,
): boolean =>
  authority.authorityBoundary === 'contract_only' &&
  authority.authEscalationDenied &&
  authority.noDeliveryQueueEnqueue &&
  authority.noExternalWebhookCall &&
  authority.noPaymentMutation &&
  authority.noProgramRunMutation &&
  authority.noReceiptMutation &&
  authority.noSecretMaterial

const assertProgramRunReceiptWebhookSafe = (
  contract: ProgramRunReceiptWebhookSubscriptionContract,
): void => {
  assertSafeRefs('program run receipt webhook identity refs', [
    contract.id,
    contract.subscriptionRef,
  ])
  assertSafeRefs('program run receipt webhook subscriber refs', contract.subscriberRefs)
  assertSafeRefs('program run receipt webhook event topic refs', contract.eventTopicRefs)
  assertSafeRefs('program run receipt webhook scoped auth refs', contract.scopedAuthRefs)
  assertSafeRefs('program run receipt webhook endpoint refs', contract.endpointRefs)
  assertSafeRefs(
    'program run receipt webhook delivery preparation refs',
    contract.deliveryPreparationRefs,
  )
  assertSafeRefs(
    'program run receipt webhook delivery attempt refs',
    contract.deliveryAttemptRefs,
  )
  assertSafeRefs('program run receipt webhook retry state refs', contract.retryStateRefs)
  assertSafeRefs('program run receipt webhook replay window refs', contract.replayWindowRefs)
  assertSafeRefs(
    'program run receipt webhook redaction policy refs',
    contract.redactionPolicyRefs,
  )
  assertSafeRefs('program run receipt webhook receipt refs', contract.receiptRefs)
  assertSafeRefs('program run receipt webhook blocker refs', contract.blockerRefs)
  assertSafeRefs('program run receipt webhook caveat refs', contract.caveatRefs)
  assertSafeRefs('program run receipt webhook evidence refs', contract.evidenceRefs)
  assertSafeRefs(
    'program run receipt webhook operator diagnostic refs',
    contract.operatorDiagnosticRefs,
  )
  assertSafeRefs('program run receipt webhook revocation refs', contract.revocationRefs)
  assertSafeRefs('program run receipt webhook idempotency refs', contract.idempotencyRefs)
  assertSafeRefs(
    'program run receipt webhook last event refs',
    contract.lastEventRef === null ? [] : [contract.lastEventRef],
  )

  if (contract.eventFamilies.length === 0) {
    throw new WebhookSubscriptionUnsafe({
      reason: 'Program Run and receipt webhook subscriptions require at least one event family.',
    })
  }

  const unsupportedFamily = contract.eventFamilies
    .find(family => !programRunReceiptWebhookAllowedFamilies.has(family))

  if (unsupportedFamily !== undefined) {
    throw new WebhookSubscriptionUnsafe({
      reason: 'Program Run and receipt webhook subscriptions can only select program_run and receipt event families.',
    })
  }

  if (!Number.isInteger(contract.maxAttempts) || contract.maxAttempts < 0) {
    throw new WebhookSubscriptionUnsafe({
      reason: 'Program Run and receipt webhook max attempts must be a non-negative integer.',
    })
  }

  if (!programRunReceiptWebhookAuthorityIsContractOnly(contract.authority)) {
    throw new WebhookSubscriptionUnsafe({
      reason: 'Program Run and receipt webhook contracts cannot call external webhooks, enqueue delivery, mutate Program Runs, mutate receipts, mutate payments, escalate auth, or carry secret material.',
    })
  }
}

const projectionText = (
  projection:
    | WebhookSubscriptionProjection
    | WebhookEventProjection
    | WebhookDeliveryProjection
    | ProgramRunReceiptWebhookProjection,
): string => JSON.stringify(projection)

export const webhookProjectionHasPrivateMaterial = (
  projection:
    | WebhookSubscriptionProjection
    | WebhookEventProjection
    | WebhookDeliveryProjection
    | ProgramRunReceiptWebhookProjection,
): boolean => {
  const text = projectionText(projection)
  const pattern = audienceUnsafePattern(projection.audience)

  return unsafeWebhookRefPattern.test(text) ||
    isoTimestampPattern.test(text) ||
    (pattern !== null && pattern.test(text))
}

export const webhookEventReplayKeyForDelivery = (
  subscriptionRef: string,
  eventRef: string,
): string => `webhook.replay:${subscriptionRef}:${eventRef}`

export const webhookDeliveryIdempotencyKey = (
  subscriptionRef: string,
  eventRef: string,
  attempt: number,
): string => `webhook.delivery:${subscriptionRef}:${eventRef}:attempt_${attempt}`

export const webhookDeliveryCanRetry = (
  delivery: WebhookDeliveryRecord,
): boolean =>
  (delivery.state === 'failed' || delivery.state === 'retry_scheduled') &&
  delivery.retryPolicy !== 'none' &&
  delivery.attempt < delivery.maxAttempts &&
  !nonRetriableFailures.has(delivery.failureClass)

export const programRunReceiptWebhookReplayKey = (
  subscriptionRef: string,
  topicRef: string,
): string => `program_run_receipt_webhook.replay:${subscriptionRef}:${topicRef}`

export const programRunReceiptWebhookIdempotencyRef = (
  subscriptionRef: string,
  topicRef: string,
  lifecycleState: ProgramRunReceiptWebhookLifecycleState,
): string =>
  `program_run_receipt_webhook.idempotency:${subscriptionRef}:${topicRef}:${lifecycleState}`

export const projectWebhookSubscription = (
  subscription: WebhookSubscriptionRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): WebhookSubscriptionProjection => {
  assertSubscriptionSafe(subscription)

  const projection: WebhookSubscriptionProjection = {
    authMode: subscription.authMode,
    audience,
    caveatRefs: safeRefsForAudience(
      'webhook subscription caveat refs',
      subscription.caveatRefs,
      audience,
    ),
    endpointRef: safeRefForAudience(
      'webhook endpoint',
      subscription.endpointRef,
      audience,
    ),
    eventFamilies: [...new Set(subscription.eventFamilies)].sort(),
    eventSourceRefs: safeRefsForAudience(
      'webhook subscription event source refs',
      subscription.eventSourceRefs,
      audience,
    ),
    evidenceRefs: safeRefsForAudience(
      'webhook subscription evidence refs',
      subscription.evidenceRefs,
      audience,
    ),
    id: safeRefForAudience('webhook subscription id', subscription.id, audience),
    maxAttempts: subscription.maxAttempts,
    ownerRef: safeRefForAudience(
      'webhook subscription owner',
      subscription.ownerRef,
      audience,
    ),
    redactionAudience: subscription.redactionAudience,
    retryPolicy: subscription.retryPolicy,
    scopeRefs: safeRefsForAudience(
      'webhook subscription scope refs',
      subscription.scopeRefs,
      audience,
    ),
    secretBindingRef: audience === 'operator'
      ? maybeSafeRefForAudience(
        'webhook subscription secret binding',
        subscription.secretBindingRef,
        audience,
      )
      : null,
    status: subscription.status,
    statusLabel: subscriptionStatusLabel[subscription.status],
    subscriptionRef: safeRefForAudience(
      'webhook subscription ref',
      subscription.subscriptionRef,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      subscription.updatedAtIso,
      nowIso,
    ),
  }

  if (webhookProjectionHasPrivateMaterial(projection)) {
    throw new WebhookSubscriptionUnsafe({
      reason: 'Webhook subscription projection contains material unsafe for the target audience.',
    })
  }

  return projection
}

export const projectProgramRunReceiptWebhook = (
  contract: ProgramRunReceiptWebhookSubscriptionContract,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): ProgramRunReceiptWebhookProjection => {
  assertProgramRunReceiptWebhookSafe(contract)

  const operatorOnlyScopedAuthRefs = audience === 'operator'
    ? safeRefsForAudience(
      'program run receipt webhook scoped auth refs',
      contract.scopedAuthRefs,
      audience,
    )
    : []
  const operatorOnlyDiagnosticRefs = audience === 'operator'
    ? safeRefsForAudience(
      'program run receipt webhook operator diagnostic refs',
      contract.operatorDiagnosticRefs,
      audience,
    )
    : []
  const projection: ProgramRunReceiptWebhookProjection = {
    audience,
    authority: contract.authority,
    authEscalationAllowed: false,
    blockerRefs: safeRefsForAudience(
      'program run receipt webhook blocker refs',
      contract.blockerRefs,
      audience,
    ),
    caveatRefs: safeRefsForAudience(
      'program run receipt webhook caveat refs',
      contract.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      contract.createdAtIso,
      nowIso,
    ),
    deliveryAttemptRecorded: contract.deliveryAttemptRefs.length > 0,
    deliveryAttemptRefs: safeRefsForAudience(
      'program run receipt webhook delivery attempt refs',
      contract.deliveryAttemptRefs,
      audience,
    ),
    deliveryPreparationRecorded: contract.deliveryPreparationRefs.length > 0,
    deliveryPreparationRefs: safeRefsForAudience(
      'program run receipt webhook delivery preparation refs',
      contract.deliveryPreparationRefs,
      audience,
    ),
    deliveryQueueEnqueueAllowed: false,
    endpointRefs: safeRefsForAudience(
      'program run receipt webhook endpoint refs',
      contract.endpointRefs,
      audience,
    ),
    eventFamilies: [...new Set(contract.eventFamilies)].sort(),
    eventSelectionRecorded: contract.eventTopicRefs.length > 0,
    eventTopicRefs: safeRefsForAudience(
      'program run receipt webhook event topic refs',
      contract.eventTopicRefs,
      audience,
    ),
    evidenceRefs: safeRefsForAudience(
      'program run receipt webhook evidence refs',
      contract.evidenceRefs,
      audience,
    ),
    externalWebhookCallAllowed: false,
    id: safeRefForAudience('program run receipt webhook id', contract.id, audience),
    idempotencyRefs: safeRefsForAudience(
      'program run receipt webhook idempotency refs',
      contract.idempotencyRefs,
      audience,
    ),
    lastEventRef: maybeSafeRefForAudience(
      'program run receipt webhook last event',
      contract.lastEventRef,
      audience,
    ),
    lifecycleLabel:
      programRunReceiptWebhookLifecycleLabel[contract.lifecycleState],
    lifecycleState: contract.lifecycleState,
    maxAttempts: contract.maxAttempts,
    operatorDiagnosticRefs: operatorOnlyDiagnosticRefs,
    paymentMutationAllowed: false,
    programRunMutationAllowed: false,
    receiptMutationAllowed: false,
    receiptRecorded: contract.receiptRefs.length > 0,
    receiptRefs: safeRefsForAudience(
      'program run receipt webhook receipt refs',
      contract.receiptRefs,
      audience,
    ),
    redactionAudience: contract.redactionAudience,
    redactionPolicyRefs: safeRefsForAudience(
      'program run receipt webhook redaction policy refs',
      contract.redactionPolicyRefs,
      audience,
    ),
    replayWindowRecorded: contract.replayWindowRefs.length > 0,
    replayWindowRefs: safeRefsForAudience(
      'program run receipt webhook replay window refs',
      contract.replayWindowRefs,
      audience,
    ),
    retryPolicy: contract.retryPolicy,
    retryScheduled: contract.retryStateRefs.length > 0,
    retryStateRefs: safeRefsForAudience(
      'program run receipt webhook retry state refs',
      contract.retryStateRefs,
      audience,
    ),
    revoked:
      contract.status === 'revoked' ||
      contract.lifecycleState === 'revoked' ||
      contract.revocationRefs.length > 0,
    revocationRefs: safeRefsForAudience(
      'program run receipt webhook revocation refs',
      contract.revocationRefs,
      audience,
    ),
    scopedAuthRefs: operatorOnlyScopedAuthRefs,
    secretMaterialAllowed: false,
    status: contract.status,
    statusLabel: subscriptionStatusLabel[contract.status],
    subscriberRefs: safeRefsForAudience(
      'program run receipt webhook subscriber refs',
      contract.subscriberRefs,
      audience,
    ),
    subscriptionRef: safeRefForAudience(
      'program run receipt webhook subscription',
      contract.subscriptionRef,
      audience,
    ),
    subscriptionRegistrationRecorded: true,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      contract.updatedAtIso,
      nowIso,
    ),
  }

  if (webhookProjectionHasPrivateMaterial(projection)) {
    throw new WebhookSubscriptionUnsafe({
      reason: 'Program Run and receipt webhook projection contains material unsafe for the target audience.',
    })
  }

  return projection
}

export const projectWebhookEvent = (
  event: WebhookEventRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): WebhookEventProjection => {
  assertEventSafe(event)

  const projection: WebhookEventProjection = {
    audience,
    caveatRefs: safeRefsForAudience(
      'webhook event caveat refs',
      event.caveatRefs,
      audience,
    ),
    eventRef: safeRefForAudience('webhook event ref', event.eventRef, audience),
    family: event.family,
    id: safeRefForAudience('webhook event id', event.id, audience),
    idempotencyKey: safeRefForAudience(
      'webhook event idempotency key',
      event.idempotencyKey,
      audience,
    ),
    occurredAtDisplay: friendlyBlueprintMissionBriefingTime(
      event.occurredAtIso,
      nowIso,
    ),
    payloadDigestRef: safeRefForAudience(
      'webhook event payload digest',
      event.payloadDigestRef,
      audience,
    ),
    payloadSchemaRef: safeRefForAudience(
      'webhook event payload schema',
      event.payloadSchemaRef,
      audience,
    ),
    receiptRefs: safeRefsForAudience(
      'webhook event receipt refs',
      event.receiptRefs,
      audience,
    ),
    redactionAudience: event.redactionAudience,
    replayKey: safeRefForAudience(
      'webhook event replay key',
      event.replayKey,
      audience,
    ),
    sourceAuthorityRefs: safeRefsForAudience(
      'webhook event source authority refs',
      event.sourceAuthorityRefs,
      audience,
    ),
    sourceRef: safeRefForAudience('webhook event source', event.sourceRef, audience),
    status: event.status,
    statusLabel: eventStatusLabel[event.status],
    subjectRef: safeRefForAudience(
      'webhook event subject',
      event.subjectRef,
      audience,
    ),
  }

  if (webhookProjectionHasPrivateMaterial(projection)) {
    throw new WebhookSubscriptionUnsafe({
      reason: 'Webhook event projection contains material unsafe for the target audience.',
    })
  }

  return projection
}

export const projectWebhookDelivery = (
  delivery: WebhookDeliveryRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): WebhookDeliveryProjection => {
  assertDeliverySafe(delivery)

  const projection: WebhookDeliveryProjection = {
    attempt: delivery.attempt,
    audience,
    canRetry: webhookDeliveryCanRetry(delivery),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      delivery.createdAtIso,
      nowIso,
    ),
    deliveredAtDisplay: delivery.deliveredAtIso === null
      ? null
      : friendlyBlueprintMissionBriefingTime(delivery.deliveredAtIso, nowIso),
    deliveryReceiptRefs: safeRefsForAudience(
      'webhook delivery receipt refs',
      delivery.deliveryReceiptRefs,
      audience,
    ),
    endpointRef: safeRefForAudience(
      'webhook delivery endpoint',
      delivery.endpointRef,
      audience,
    ),
    eventRef: safeRefForAudience('webhook delivery event', delivery.eventRef, audience),
    failureClass: delivery.failureClass,
    failureClassLabel: failureClassLabel[delivery.failureClass],
    failureSummaryRef: maybeSafeRefForAudience(
      'webhook delivery failure summary',
      delivery.failureSummaryRef,
      audience,
    ),
    id: safeRefForAudience('webhook delivery id', delivery.id, audience),
    idempotencyKey: safeRefForAudience(
      'webhook delivery idempotency key',
      delivery.idempotencyKey,
      audience,
    ),
    maxAttempts: delivery.maxAttempts,
    nextAttemptAtDisplay: delivery.nextAttemptAtIso === null
      ? null
      : friendlyBlueprintMissionBriefingTime(delivery.nextAttemptAtIso, nowIso),
    payloadDigestRef: safeRefForAudience(
      'webhook delivery payload digest',
      delivery.payloadDigestRef,
      audience,
    ),
    redactionAudience: delivery.redactionAudience,
    replayKey: safeRefForAudience(
      'webhook delivery replay key',
      delivery.replayKey,
      audience,
    ),
    retryPolicy: delivery.retryPolicy,
    state: delivery.state,
    stateLabel: deliveryStateLabel[delivery.state],
    subscriptionRef: safeRefForAudience(
      'webhook delivery subscription',
      delivery.subscriptionRef,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      delivery.updatedAtIso,
      nowIso,
    ),
  }

  if (webhookProjectionHasPrivateMaterial(projection)) {
    throw new WebhookSubscriptionUnsafe({
      reason: 'Webhook delivery projection contains material unsafe for the target audience.',
    })
  }

  return projection
}

export const exampleWebhookSubscription = (): WebhookSubscriptionRecord => ({
  authMode: 'owner_grant',
  caveatRefs: ['caveat.webhook.redacted_payload_only'],
  createdAtIso: '2026-06-06T22:20:00.000Z',
  endpointRef: 'endpoint.public.customer_ops',
  eventFamilies: ['workroom', 'site_revision', 'receipt'],
  eventSourceRefs: ['source.webhook.workroom_events'],
  evidenceRefs: ['evidence.webhook.subscription_contract_v1'],
  id: 'webhook_subscription.customer_ops',
  maxAttempts: 3,
  ownerRef: 'owner.team.openagents_core',
  redactionAudience: 'customer',
  retryPolicy: 'exponential',
  scopeRefs: ['scope.customer_order.software_order_otec'],
  secretBindingRef: 'secret_binding.webhook.customer_ops',
  status: 'active',
  subscriptionRef: 'webhook.sub.customer_ops',
  updatedAtIso: '2026-06-06T22:25:00.000Z',
})

export const exampleWebhookEvent = (): WebhookEventRecord => ({
  caveatRefs: ['caveat.webhook.event_public_safe'],
  eventRef: 'event.webhook.site_revision_ready',
  family: 'site_revision',
  id: 'webhook_event.site_revision_ready',
  idempotencyKey: 'webhook.event.site_revision_ready',
  occurredAtIso: '2026-06-06T22:26:00.000Z',
  payloadDigestRef: 'digest.webhook_payload.site_revision_ready',
  payloadSchemaRef: 'schema.webhook.site_revision_ready.v1',
  receiptRefs: ['receipt.site_revision.ready'],
  redactionAudience: 'customer',
  replayKey: 'webhook.replay:webhook.sub.customer_ops:event.webhook.site_revision_ready',
  sourceAuthorityRefs: ['source_authority.site_revision.lifecycle'],
  sourceRef: 'source.site_revision.site_otec_v4',
  status: 'ready',
  subjectRef: 'subject.site_revision.site_otec_v4',
})

export const exampleWebhookDelivery = (): WebhookDeliveryRecord => ({
  attempt: 1,
  createdAtIso: '2026-06-06T22:27:00.000Z',
  deliveredAtIso: null,
  deliveryReceiptRefs: [],
  endpointRef: 'endpoint.public.customer_ops',
  eventRef: 'event.webhook.site_revision_ready',
  failureClass: 'timeout',
  failureSummaryRef: 'failure.webhook.timeout_1',
  id: 'webhook_delivery.customer_ops.site_revision_ready.1',
  idempotencyKey:
    'webhook.delivery:webhook.sub.customer_ops:event.webhook.site_revision_ready:attempt_1',
  maxAttempts: 3,
  nextAttemptAtIso: '2026-06-06T22:35:00.000Z',
  payloadDigestRef: 'digest.webhook_payload.site_revision_ready',
  redactionAudience: 'customer',
  replayKey: 'webhook.replay:webhook.sub.customer_ops:event.webhook.site_revision_ready',
  retryPolicy: 'exponential',
  state: 'retry_scheduled',
  subscriptionRef: 'webhook.sub.customer_ops',
  updatedAtIso: '2026-06-06T22:28:00.000Z',
})

export const programRunReceiptWebhookContractOnlyAuthority =
  (): ProgramRunReceiptWebhookAuthority => ({
    authEscalationDenied: true,
    authorityBoundary: 'contract_only',
    noDeliveryQueueEnqueue: true,
    noExternalWebhookCall: true,
    noPaymentMutation: true,
    noProgramRunMutation: true,
    noReceiptMutation: true,
    noSecretMaterial: true,
  })

export const exampleProgramRunReceiptWebhook =
  (): ProgramRunReceiptWebhookSubscriptionContract => ({
    authority: programRunReceiptWebhookContractOnlyAuthority(),
    blockerRefs: [],
    caveatRefs: ['caveat.webhook.no_external_send'],
    createdAtIso: '2026-06-06T22:15:00.000Z',
    deliveryAttemptRefs: ['delivery_attempt.program_run_receipt.1'],
    deliveryPreparationRefs: ['delivery_preparation.program_run_receipt.v1'],
    endpointRefs: [
      'endpoint.public.partner_receipts',
      'endpoint.private.customer_ops',
    ],
    eventFamilies: ['program_run', 'receipt'],
    eventTopicRefs: [
      'topic.program_run.lifecycle',
      'topic.receipt.lifecycle',
    ],
    evidenceRefs: ['evidence.webhook.program_run_receipt_contract_v1'],
    id: 'program_run_receipt_webhook.customer_ops',
    idempotencyRefs: [
      'program_run_receipt_webhook.idempotency:webhook.sub.program_run_receipt:topic.receipt.lifecycle:receipt_recorded',
    ],
    lastEventRef: 'event.program_run_receipt.receipt_recorded',
    lifecycleState: 'receipt_recorded',
    maxAttempts: 3,
    operatorDiagnosticRefs: ['diagnostic.operator.webhook.program_run_receipt'],
    receiptRefs: ['receipt.program_run.outcome_recorded'],
    redactionAudience: 'team',
    redactionPolicyRefs: ['redaction_policy.program_run_receipt.public_safe'],
    replayWindowRefs: ['replay_window.program_run_receipt.seven_days'],
    retryPolicy: 'exponential',
    retryStateRefs: ['retry_state.program_run_receipt.next_attempt'],
    revocationRefs: [],
    scopedAuthRefs: ['auth.private.owner_grant.customer_ops'],
    status: 'active',
    subscriberRefs: [
      'subscriber.public.partner_receipts',
      'subscriber.private.customer_ops',
    ],
    subscriptionRef: 'webhook.sub.program_run_receipt',
    updatedAtIso: '2026-06-06T22:25:00.000Z',
  })
