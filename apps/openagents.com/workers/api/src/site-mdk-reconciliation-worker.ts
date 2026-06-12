import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  BuyerPaymentChallengeRecord,
  BuyerPaymentEntitlementRecord,
  BuyerPaymentReceiptRecord,
  BuyerPaymentReconciliationEventRecord,
} from './buyer-payment-ledger'
import { OpenAgentsHostedMdkCheckoutStatus } from './hosted-mdk-client'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'
import { OpenAgentsSiteMdkCheckoutIntentRecord } from './site-mdk-checkout-intents'
import { OpenAgentsSiteMdkProviderEvent } from './site-mdk-reconciliation'

export const OpenAgentsSiteMdkReconciliationWorkerSource = S.Literals([
  'operator',
  'queue',
  'scheduled',
  'webhook',
])
export type OpenAgentsSiteMdkReconciliationWorkerSource =
  typeof OpenAgentsSiteMdkReconciliationWorkerSource.Type

export const OpenAgentsSiteMdkReconciliationWorkerStatus = S.Literals([
  'conflict',
  'entitlement_created',
  'expired',
  'operator_review',
  'payment_seen',
  'pending',
  'provider_seen',
  'provider_unavailable',
  'receipt_created',
  'replayed',
  'stale',
])
export type OpenAgentsSiteMdkReconciliationWorkerStatus =
  typeof OpenAgentsSiteMdkReconciliationWorkerStatus.Type

export const OpenAgentsSiteMdkReconciliationWorkerAction = S.Literals([
  'create_entitlement_once',
  'create_receipt_once',
  'expire_checkout_intent',
  'expire_payment_challenge',
  'record_reconciliation_event_once',
  'request_operator_review',
  'schedule_status_check',
])
export type OpenAgentsSiteMdkReconciliationWorkerAction =
  typeof OpenAgentsSiteMdkReconciliationWorkerAction.Type

export const OpenAgentsSiteMdkProviderStatusCheck = S.Struct({
  checkedAt: S.String,
  checkoutRef: S.String,
  checkoutStatus: OpenAgentsHostedMdkCheckoutStatus,
  eventBodyDigestRef: S.NullOr(S.String),
  providerAvailable: S.Boolean,
  providerEventRef: S.NullOr(S.String),
  providerRef: S.String,
  statusCheckRef: S.String,
  statusCheckSupported: S.Boolean,
})
export type OpenAgentsSiteMdkProviderStatusCheck =
  typeof OpenAgentsSiteMdkProviderStatusCheck.Type

export const OpenAgentsSiteMdkReconciliationWorkerRetryPlan = S.Struct({
  attempt: S.Number,
  backoffSeconds: S.Number,
  maxAttempts: S.Number,
  nextAttemptAt: S.NullOr(S.String),
})
export type OpenAgentsSiteMdkReconciliationWorkerRetryPlan =
  typeof OpenAgentsSiteMdkReconciliationWorkerRetryPlan.Type

export const OpenAgentsSiteMdkReconciliationWorkerSideEffects = S.Struct({
  callsProviderStatusApi: S.Boolean,
  createsEntitlement: S.Boolean,
  createsReceipt: S.Boolean,
  mutatesPayout: S.Boolean,
  recordsReconciliationEvent: S.Boolean,
  updatesCheckoutIntent: S.Boolean,
  updatesPaymentChallenge: S.Boolean,
})
export type OpenAgentsSiteMdkReconciliationWorkerSideEffects =
  typeof OpenAgentsSiteMdkReconciliationWorkerSideEffects.Type

export const OpenAgentsSiteMdkReconciliationWorkerInput = S.Struct({
  audience: OpenAgentsPaymentPolicyAudience,
  buyerPaymentChallenge: BuyerPaymentChallengeRecord,
  checkoutIntent: OpenAgentsSiteMdkCheckoutIntentRecord,
  entitlement: S.NullOr(BuyerPaymentEntitlementRecord),
  existingReconciliationEvents: S.Array(BuyerPaymentReconciliationEventRecord),
  incomingProviderEvent: S.NullOr(OpenAgentsSiteMdkProviderEvent),
  nowIso: S.String,
  providerStatusCheck: S.NullOr(OpenAgentsSiteMdkProviderStatusCheck),
  receipt: S.NullOr(BuyerPaymentReceiptRecord),
  retryPlan: OpenAgentsSiteMdkReconciliationWorkerRetryPlan,
  source: OpenAgentsSiteMdkReconciliationWorkerSource,
  staleAfterSeconds: S.Number,
})
export type OpenAgentsSiteMdkReconciliationWorkerInput =
  typeof OpenAgentsSiteMdkReconciliationWorkerInput.Type

export const OpenAgentsSiteMdkReconciliationWorkerProjection = S.Struct({
  actionRefs: S.Array(OpenAgentsSiteMdkReconciliationWorkerAction),
  audience: OpenAgentsPaymentPolicyAudience,
  challengeRef: S.String,
  checkoutIntentRef: S.String,
  checkoutRef: S.String,
  conflictRefs: S.Array(S.String),
  duplicateRefs: S.Array(S.String),
  entitlementRef: S.NullOr(S.String),
  idempotencyRefs: S.Array(S.String),
  nextAttemptAt: S.NullOr(S.String),
  operatorRefs: S.Array(S.String),
  productId: S.String,
  providerRef: S.String,
  publicProjectionJson: S.String,
  reasonRefs: S.Array(S.String),
  receiptRef: S.NullOr(S.String),
  retryAllowed: S.Boolean,
  safeBody: S.Record(S.String, S.Unknown),
  sideEffectSummary: OpenAgentsSiteMdkReconciliationWorkerSideEffects,
  source: OpenAgentsSiteMdkReconciliationWorkerSource,
  status: OpenAgentsSiteMdkReconciliationWorkerStatus,
  statusCode: S.Number,
})
export type OpenAgentsSiteMdkReconciliationWorkerProjection =
  typeof OpenAgentsSiteMdkReconciliationWorkerProjection.Type

export class OpenAgentsSiteMdkReconciliationWorkerUnsafe extends S.TaggedErrorClass<OpenAgentsSiteMdkReconciliationWorkerUnsafe>()(
  'OpenAgentsSiteMdkReconciliationWorkerUnsafe',
  {
    reason: S.String,
  },
) {}

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const timestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const timestampKeys = new Set([
  'checkedAt',
  'createdAt',
  'expiresAt',
  'nextAttemptAt',
  'nowIso',
  'occurredAt',
  'updatedAt',
])
const publicLiteralValues = new Set([
  'active',
  'agent',
  'bitcoin',
  'bitcoin_millisatoshi',
  'cancelled',
  'conflict',
  'create_entitlement_once',
  'create_receipt_once',
  'created',
  'credit',
  'credits',
  'entitlement_created',
  'expire_checkout_intent',
  'expire_payment_challenge',
  'expired',
  'fake_provider_only',
  'issued',
  'matched',
  'operator',
  'operator_review',
  'payment_pending',
  'payment_received',
  'payment_seen',
  'pending',
  'pending_payment',
  'production',
  'provider_seen',
  'provider_unavailable',
  'queue',
  'receipt_created',
  'record_reconciliation_event_once',
  'rejected',
  'replayed',
  'request_operator_review',
  'sandbox',
  'scheduled',
  'schedule_status_check',
  'site_checkout',
  'stale',
  'usd',
  'usd_cent',
  'verification_config_gated',
  'webhook',
])
const unsafeKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|checkout[_-]?id|cookie|customer[_-]?(email|name|id|value)|email[_-]?body|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|method|preimage|proof)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|source[_-]?archive|stripe[_-]?(customer|invoice|payment|secret|webhook)|wallet)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|cus_[A-Za-z0-9]+|evt_[A-Za-z0-9]+|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|in_[A-Za-z0-9]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|method|preimage|proof)|pm_[A-Za-z0-9]+|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|sk-[a-z0-9]|wallet[_-]?state|whsec_[A-Za-z0-9]+|\S+@\S+)/i

const scanForUnsafeWorkerMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    const key = path.at(-1)

    if (publicLiteralValues.has(value)) {
      return undefined
    }

    if (
      key !== undefined &&
      timestampKeys.has(key) &&
      timestampPattern.test(value)
    ) {
      return undefined
    }

    return containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
      ? path.join('.') || '<root>'
      : undefined
  }

  if (Array.isArray(value)) {
    return value
      .map((item, index) =>
        scanForUnsafeWorkerMaterial(item, [...path, String(index)]),
      )
      .find((unsafePath): unsafePath is string => unsafePath !== undefined)
  }

  if (value === null || typeof value !== 'object') {
    return undefined
  }

  if (openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)) {
    return path.join('.') || '<root>'
  }

  return Object.entries(value)
    .map(([key, item]) =>
      unsafeKeyPattern.test(key)
        ? [...path, key].join('.')
        : scanForUnsafeWorkerMaterial(item, [...path, key]),
    )
    .find((unsafePath): unsafePath is string => unsafePath !== undefined)
}

const assertSafeWorkerValue = (label: string, value: unknown): void => {
  const unsafePath = scanForUnsafeWorkerMaterial(value)

  if (unsafePath !== undefined) {
    throw new OpenAgentsSiteMdkReconciliationWorkerUnsafe({
      reason: `${label} contains private or payment-secret material at ${unsafePath}.`,
    })
  }
}

const safeRef = (value: string | null | undefined): string | undefined =>
  value !== null &&
  value !== undefined &&
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  scanForUnsafeWorkerMaterial(value) === undefined
    ? value
    : undefined

const safeRefs = (refs: ReadonlyArray<string | null | undefined>): string[] =>
  [...new Set(refs)]
    .map(ref => safeRef(ref))
    .filter((ref): ref is string => ref !== undefined)

const nowMillis = (input: OpenAgentsSiteMdkReconciliationWorkerInput): number =>
  Date.parse(input.nowIso)

const timestampMillis = (value: string): number => {
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : 0
}

const checkoutMatchesChallenge = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): boolean =>
  input.checkoutIntent.challengeRef === input.buyerPaymentChallenge.challengeRef &&
  input.checkoutIntent.productId === input.buyerPaymentChallenge.productId

const receiptMatches = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): boolean =>
  input.receipt === null ||
  (
    input.receipt.challengeRef === input.buyerPaymentChallenge.challengeRef &&
    input.receipt.productId === input.checkoutIntent.productId
  )

const entitlementMatches = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): boolean =>
  input.entitlement === null ||
  (
    input.entitlement.challengeRef === input.buyerPaymentChallenge.challengeRef &&
    input.entitlement.productId === input.checkoutIntent.productId &&
    (input.receipt === null ||
      input.entitlement.receiptRef === input.receipt.receiptRef)
  )

const eventMatches = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): boolean =>
  input.incomingProviderEvent === null ||
  (
    input.incomingProviderEvent.challengeRef ===
      input.buyerPaymentChallenge.challengeRef &&
    input.incomingProviderEvent.checkoutRef ===
      input.checkoutIntent.checkoutRef &&
    input.incomingProviderEvent.productId === input.checkoutIntent.productId &&
    input.incomingProviderEvent.providerRef === input.checkoutIntent.providerRef &&
    input.incomingProviderEvent.siteId === input.checkoutIntent.siteId &&
    input.incomingProviderEvent.siteVersionId ===
      input.checkoutIntent.siteVersionId
  )

const statusCheckMatches = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): boolean =>
  input.providerStatusCheck === null ||
  (
    input.providerStatusCheck.checkoutRef === input.checkoutIntent.checkoutRef &&
    input.providerStatusCheck.providerRef === input.checkoutIntent.providerRef
  )

const duplicatedEventRefs = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): string[] => {
  const event = input.incomingProviderEvent

  if (event === null) {
    return []
  }

  return safeRefs(
    input.existingReconciliationEvents
      .filter(record =>
        record.providerRef === event.providerRef &&
        (
          record.externalEventRef === event.providerEventRef ||
          record.idempotencyKeyHash === event.eventBodyDigestRef ||
          record.challengeRef === event.challengeRef &&
            record.receiptRef !== null &&
            input.receipt?.receiptRef === record.receiptRef
        ),
      )
      .flatMap(record => [record.eventRef, record.externalEventRef]),
  )
}

const paymentObserved = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): boolean =>
  input.checkoutIntent.status === 'payment_received' ||
  input.receipt !== null ||
  input.incomingProviderEvent?.checkoutStatus === 'payment_received' ||
  input.providerStatusCheck?.checkoutStatus === 'payment_received'

const providerObserved = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): boolean =>
  input.incomingProviderEvent !== null ||
  input.providerStatusCheck !== null

const challengeExpired = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): boolean =>
  !paymentObserved(input) &&
  (
    input.buyerPaymentChallenge.status === 'expired' ||
    input.checkoutIntent.status === 'expired' ||
    timestampMillis(input.buyerPaymentChallenge.expiresAt) <= nowMillis(input)
  )

const checkoutStale = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): boolean =>
  !paymentObserved(input) &&
  !providerObserved(input) &&
  timestampMillis(input.checkoutIntent.updatedAt) +
    input.staleAfterSeconds * 1000 <= nowMillis(input)

const providerUnavailable = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): boolean =>
  input.providerStatusCheck !== null &&
  !input.providerStatusCheck.providerAvailable

const nonFakeEventNeedsVerification = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): boolean =>
  input.incomingProviderEvent !== null &&
  !input.incomingProviderEvent.fakeProvider &&
  !input.incomingProviderEvent.signatureVerified

const outOfOrderProviderStatus = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): boolean =>
  input.providerStatusCheck !== null &&
  input.checkoutIntent.status === 'payment_received' &&
  input.providerStatusCheck.checkoutStatus !== 'payment_received'

const conflictRefs = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): string[] =>
  safeRefs([
    !checkoutMatchesChallenge(input)
      ? 'conflict.site_mdk_worker.checkout_challenge_mismatch'
      : null,
    !receiptMatches(input)
      ? 'conflict.site_mdk_worker.receipt_mismatch'
      : null,
    !entitlementMatches(input)
      ? 'conflict.site_mdk_worker.entitlement_mismatch'
      : null,
    !eventMatches(input)
      ? 'conflict.site_mdk_worker.provider_event_mismatch'
      : null,
    !statusCheckMatches(input)
      ? 'conflict.site_mdk_worker.status_check_mismatch'
      : null,
    outOfOrderProviderStatus(input)
      ? 'conflict.site_mdk_worker.out_of_order_provider_status'
      : null,
  ])

const statusForInput = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): OpenAgentsSiteMdkReconciliationWorkerStatus => {
  const duplicates = duplicatedEventRefs(input)
  const conflicts = conflictRefs(input)

  if (duplicates.length > 0) {
    return 'replayed'
  }

  if (conflicts.length > 0) {
    return 'conflict'
  }

  if (nonFakeEventNeedsVerification(input)) {
    return 'operator_review'
  }

  if (providerUnavailable(input)) {
    return 'provider_unavailable'
  }

  if (paymentObserved(input)) {
    if (input.receipt === null) {
      return 'receipt_created'
    }

    if (input.entitlement === null) {
      return 'entitlement_created'
    }

    return 'payment_seen'
  }

  if (challengeExpired(input)) {
    return 'expired'
  }

  if (checkoutStale(input)) {
    return 'stale'
  }

  if (providerObserved(input)) {
    return 'provider_seen'
  }

  return 'pending'
}

const statusCodeForStatus = (
  status: OpenAgentsSiteMdkReconciliationWorkerStatus,
): number =>
  status === 'conflict'
    ? 409
    : status === 'provider_unavailable'
      ? 503
      : status === 'operator_review'
        ? 202
        : 200

const actionsForStatus = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
  status: OpenAgentsSiteMdkReconciliationWorkerStatus,
): OpenAgentsSiteMdkReconciliationWorkerAction[] =>
  status === 'receipt_created'
    ? ['record_reconciliation_event_once', 'create_receipt_once']
    : status === 'entitlement_created'
      ? ['record_reconciliation_event_once', 'create_entitlement_once']
      : status === 'payment_seen'
        ? ['record_reconciliation_event_once']
        : status === 'expired'
          ? ['expire_payment_challenge', 'expire_checkout_intent']
          : status === 'stale'
            ? input.providerStatusCheck?.statusCheckSupported === true
              ? ['schedule_status_check']
              : ['request_operator_review']
            : status === 'operator_review' || status === 'conflict'
              ? ['request_operator_review']
              : status === 'provider_seen'
                ? ['record_reconciliation_event_once']
                : []

const retryAllowed = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
  status: OpenAgentsSiteMdkReconciliationWorkerStatus,
): boolean =>
  (
    status === 'pending' ||
    status === 'provider_seen' ||
    status === 'provider_unavailable' ||
    status === 'stale'
  ) &&
  input.retryPlan.attempt < input.retryPlan.maxAttempts

const idempotencyRefs = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): string[] =>
  safeRefs([
    input.checkoutIntent.idempotencyKeyHash,
    input.incomingProviderEvent?.eventBodyDigestRef,
    input.providerStatusCheck?.eventBodyDigestRef,
    input.providerStatusCheck?.statusCheckRef,
    input.receipt?.receiptRef,
    input.entitlement?.entitlementRef,
  ])

const operatorRefs = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
  status: OpenAgentsSiteMdkReconciliationWorkerStatus,
): string[] =>
  input.audience === 'operator'
    ? safeRefs([
      input.checkoutIntent.checkoutIntentRef,
      input.checkoutIntent.checkoutRef,
      input.checkoutIntent.providerRef,
      input.incomingProviderEvent?.providerEventRef,
      input.incomingProviderEvent?.eventBodyDigestRef,
      input.providerStatusCheck?.providerEventRef,
      input.providerStatusCheck?.eventBodyDigestRef,
      input.providerStatusCheck?.statusCheckRef,
      ...duplicatedEventRefs(input),
      ...conflictRefs(input),
      `status.site_mdk_worker.${status}`,
    ])
    : []

const reasonRefs = (
  status: OpenAgentsSiteMdkReconciliationWorkerStatus,
): string[] => safeRefs([`reason.site_mdk_worker.${status}`])

const publicProjectionJson = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
  status: OpenAgentsSiteMdkReconciliationWorkerStatus,
): string =>
  JSON.stringify({
    checkoutIntentRef: input.checkoutIntent.checkoutIntentRef,
    checkoutRef: input.checkoutIntent.checkoutRef,
    productId: input.checkoutIntent.productId,
    siteId: input.checkoutIntent.siteId,
    siteVersionId: input.checkoutIntent.siteVersionId,
    status,
  })

const sideEffectsForActions = (
  actions: ReadonlyArray<OpenAgentsSiteMdkReconciliationWorkerAction>,
): OpenAgentsSiteMdkReconciliationWorkerSideEffects => ({
  callsProviderStatusApi: actions.includes('schedule_status_check'),
  createsEntitlement: actions.includes('create_entitlement_once'),
  createsReceipt: actions.includes('create_receipt_once'),
  mutatesPayout: false,
  recordsReconciliationEvent:
    actions.includes('record_reconciliation_event_once'),
  updatesCheckoutIntent: actions.includes('expire_checkout_intent'),
  updatesPaymentChallenge: actions.includes('expire_payment_challenge'),
})

const projectionIsSafe = (
  projection: OpenAgentsSiteMdkReconciliationWorkerProjection,
): boolean => scanForUnsafeWorkerMaterial(projection) === undefined

export const openAgentsSiteMdkReconciliationWorkerHasPrivateMaterial = (
  value: unknown,
): boolean => scanForUnsafeWorkerMaterial(value) !== undefined

export const planOpenAgentsSiteMdkReconciliationWorker = (
  input: OpenAgentsSiteMdkReconciliationWorkerInput,
): OpenAgentsSiteMdkReconciliationWorkerProjection => {
  assertSafeWorkerValue('Site MDK reconciliation worker input', input)

  const status = statusForInput(input)
  const actions = actionsForStatus(input, status)
  const projection: OpenAgentsSiteMdkReconciliationWorkerProjection = {
    actionRefs: actions,
    audience: input.audience,
    challengeRef: input.buyerPaymentChallenge.challengeRef,
    checkoutIntentRef: input.checkoutIntent.checkoutIntentRef,
    checkoutRef: input.checkoutIntent.checkoutRef,
    conflictRefs: conflictRefs(input),
    duplicateRefs: duplicatedEventRefs(input),
    entitlementRef: safeRef(input.entitlement?.entitlementRef) ?? null,
    idempotencyRefs: idempotencyRefs(input),
    nextAttemptAt: retryAllowed(input, status)
      ? safeRef(input.retryPlan.nextAttemptAt) ?? null
      : null,
    operatorRefs: operatorRefs(input, status),
    productId: input.checkoutIntent.productId,
    providerRef: input.checkoutIntent.providerRef,
    publicProjectionJson: publicProjectionJson(input, status),
    reasonRefs: reasonRefs(status),
    receiptRef: safeRef(input.receipt?.receiptRef) ?? null,
    retryAllowed: retryAllowed(input, status),
    safeBody: {
      action: 'site_mdk_reconciliation_worker',
      actionRefs: actions,
      checkoutIntentRef: input.checkoutIntent.checkoutIntentRef,
      status,
    },
    sideEffectSummary: sideEffectsForActions(actions),
    source: input.source,
    status,
    statusCode: statusCodeForStatus(status),
  }

  if (!projectionIsSafe(projection)) {
    throw new OpenAgentsSiteMdkReconciliationWorkerUnsafe({
      reason:
        'Site MDK reconciliation worker projection is not public-safe.',
    })
  }

  return projection
}
