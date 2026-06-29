import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  BuyerPaymentChallengeRecord,
  BuyerPaymentEntitlementRecord,
  BuyerPaymentLedgerProjection,
  BuyerPaymentReceiptRecord,
  projectBuyerPaymentLedgerRecord,
} from './buyer-payment-ledger'
import {
  OpenAgentsL402VerificationResult,
  type OpenAgentsL402VerificationResult as OpenAgentsL402VerificationResultType,
} from './l402-credential-service'
import {
  OpenAgentsPaymentPolicyAudience,
  OpenAgentsPaymentPolicySurface,
} from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsL402DeferredSettlementMode = S.Literals([
  'deferred_until_artifact_receipt',
  'deferred_until_response_closeout',
  'deferred_until_success',
  'immediate',
  'manual_operator_review',
])
export type OpenAgentsL402DeferredSettlementMode =
  typeof OpenAgentsL402DeferredSettlementMode.Type

export const OpenAgentsL402DeferredSettlementWorkStatus = S.Literals([
  'artifact_receipt_created',
  'failed',
  'not_started',
  'response_closed',
  'succeeded',
])
export type OpenAgentsL402DeferredSettlementWorkStatus =
  typeof OpenAgentsL402DeferredSettlementWorkStatus.Type

export const OpenAgentsL402DeferredSettlementStatus = S.Literals([
  'allow',
  'blocked',
  'payment_required',
  'retryable_failure',
  'settled',
  'settlement_pending',
])
export type OpenAgentsL402DeferredSettlementStatus =
  typeof OpenAgentsL402DeferredSettlementStatus.Type

export const OpenAgentsL402DeferredSettlementWorkResult = S.Struct({
  artifactReceiptRef: S.NullOr(S.String),
  failureRef: S.NullOr(S.String),
  responseCloseoutRef: S.NullOr(S.String),
  retryable: S.Boolean,
  status: OpenAgentsL402DeferredSettlementWorkStatus,
})
export type OpenAgentsL402DeferredSettlementWorkResult =
  typeof OpenAgentsL402DeferredSettlementWorkResult.Type

export const OpenAgentsL402DeferredSettlementInput = S.Struct({
  actorRef: S.String,
  audience: OpenAgentsPaymentPolicyAudience,
  buyerPaymentChallenge: BuyerPaymentChallengeRecord,
  endpointRef: S.String,
  existingEntitlement: S.NullOr(BuyerPaymentEntitlementRecord),
  existingReceipt: S.NullOr(BuyerPaymentReceiptRecord),
  expectedEntitlementScopeRefs: S.Array(S.String),
  idempotencyKeyHash: S.String,
  manualApprovalRef: S.NullOr(S.String),
  metadataRefs: S.Array(S.String),
  mode: OpenAgentsL402DeferredSettlementMode,
  nowIso: S.String,
  productId: S.String,
  surface: OpenAgentsPaymentPolicySurface,
  verification: S.NullOr(OpenAgentsL402VerificationResult),
  workResult: OpenAgentsL402DeferredSettlementWorkResult,
})
export type OpenAgentsL402DeferredSettlementInput =
  typeof OpenAgentsL402DeferredSettlementInput.Type

export const OpenAgentsL402DeferredSettlementAttemptRecord = S.Struct({
  actorRef: S.String,
  attemptRef: S.String,
  challengeRef: S.String,
  createdAt: S.String,
  credentialConsumed: S.Boolean,
  credentialRef: S.NullOr(S.String),
  credentialReusable: S.Boolean,
  endpointRef: S.String,
  entitlementRef: S.NullOr(S.String),
  failureRef: S.NullOr(S.String),
  idempotencyKeyHash: S.String,
  metadataRefs: S.Array(S.String),
  mode: OpenAgentsL402DeferredSettlementMode,
  productId: S.String,
  reasonRefs: S.Array(S.String),
  receiptRef: S.NullOr(S.String),
  retryable: S.Boolean,
  settlementRef: S.String,
  status: OpenAgentsL402DeferredSettlementStatus,
  surface: OpenAgentsPaymentPolicySurface,
  updatedAt: S.String,
  workResult: OpenAgentsL402DeferredSettlementWorkResult,
})
export type OpenAgentsL402DeferredSettlementAttemptRecord =
  typeof OpenAgentsL402DeferredSettlementAttemptRecord.Type

export const OpenAgentsL402DeferredSettlementProjection = S.Struct({
  actorRef: S.NullOr(S.String),
  attemptRef: S.String,
  audience: OpenAgentsPaymentPolicyAudience,
  buyerPaymentChallenge: S.NullOr(BuyerPaymentLedgerProjection),
  credentialConsumed: S.Boolean,
  credentialRef: S.NullOr(S.String),
  credentialReusable: S.Boolean,
  endpointRef: S.String,
  entitlement: S.NullOr(BuyerPaymentLedgerProjection),
  failureRef: S.NullOr(S.String),
  productId: S.String,
  reasonRefs: S.Array(S.String),
  receipt: S.NullOr(BuyerPaymentLedgerProjection),
  retryable: S.Boolean,
  safeBody: S.Record(S.String, S.Unknown),
  settlementRef: S.String,
  status: OpenAgentsL402DeferredSettlementStatus,
  statusCode: S.Number,
  surface: OpenAgentsPaymentPolicySurface,
})
export type OpenAgentsL402DeferredSettlementProjection =
  typeof OpenAgentsL402DeferredSettlementProjection.Type

export class OpenAgentsL402DeferredSettlementUnsafe extends S.TaggedErrorClass<OpenAgentsL402DeferredSettlementUnsafe>()(
  'OpenAgentsL402DeferredSettlementUnsafe',
  {
    reason: S.String,
  },
) {}

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const timestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const timestampKeys = new Set([
  'createdAt',
  'expiresAt',
  'nowIso',
  'updatedAt',
])
const unsafeKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|cookie|customer[_-]?(email|name)|email[_-]?body|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?preimage|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|source[_-]?archive|wallet|webhook)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment_hash=|payment_preimage=|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?state)/i

const scanForUnsafeSettlementMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    const key = path.at(-1)

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
        scanForUnsafeSettlementMaterial(item, [...path, String(index)]),
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
        : scanForUnsafeSettlementMaterial(item, [...path, key]),
    )
    .find((unsafePath): unsafePath is string => unsafePath !== undefined)
}

const assertSafeSettlementValue = (label: string, value: unknown): void => {
  const unsafePath = scanForUnsafeSettlementMaterial(value)

  if (unsafePath !== undefined) {
    throw new OpenAgentsL402DeferredSettlementUnsafe({
      reason: `${label} contains private or payment-secret material at ${unsafePath}.`,
    })
  }
}

const safeRef = (value: string): string | undefined =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  scanForUnsafeSettlementMaterial(value) === undefined
    ? value
    : undefined

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)]
    .map(ref => safeRef(ref))
    .filter((ref): ref is string => ref !== undefined)

const nullableSafeRef = (
  value: string | null | undefined,
): string | null =>
  value === null || value === undefined ? null : safeRef(value) ?? null

const statusCodeForStatus = (
  status: OpenAgentsL402DeferredSettlementStatus,
): number =>
  status === 'payment_required'
    ? 402
    : status === 'blocked'
      ? 403
      : status === 'settlement_pending'
        ? 202
        : status === 'retryable_failure'
          ? 500
          : 200

const activeEntitlementMatches = (
  input: OpenAgentsL402DeferredSettlementInput,
): boolean =>
  input.existingEntitlement !== null &&
  input.existingEntitlement.status === 'active' &&
  input.existingEntitlement.productId === input.productId &&
  input.expectedEntitlementScopeRefs.some(scopeRef =>
    input.existingEntitlement?.scopeRefs.includes(scopeRef),
  )

const verificationCredentialRef = (
  verification: OpenAgentsL402VerificationResultType | null,
): string | null =>
  verification?.status === 'valid' && verification.credentialRef !== null
    ? nullableSafeRef(verification.credentialRef)
    : null

const verificationIsValid = (
  verification: OpenAgentsL402VerificationResultType | null,
): boolean =>
  verification?.status === 'valid' &&
  verification.payload !== null &&
  verificationCredentialRef(verification) !== null

const challengeIsIssued = (
  input: OpenAgentsL402DeferredSettlementInput,
): boolean =>
  input.buyerPaymentChallenge.status === 'issued' &&
  input.buyerPaymentChallenge.productId === input.productId &&
  input.buyerPaymentChallenge.expiresAt > input.nowIso

const existingSettlementMatches = (
  input: OpenAgentsL402DeferredSettlementInput,
): boolean =>
  input.existingReceipt !== null &&
  input.existingReceipt.status === 'issued' &&
  input.existingReceipt.challengeRef ===
    input.buyerPaymentChallenge.challengeRef &&
  input.existingReceipt.productId === input.productId &&
  input.existingEntitlement !== null &&
  input.existingEntitlement.status === 'active' &&
  input.existingEntitlement.receiptRef === input.existingReceipt.receiptRef

const successBoundarySatisfied = (
  input: OpenAgentsL402DeferredSettlementInput,
): boolean =>
  input.mode === 'immediate' ||
  (input.mode === 'deferred_until_success' &&
    input.workResult.status === 'succeeded') ||
  (input.mode === 'deferred_until_artifact_receipt' &&
    input.workResult.status === 'artifact_receipt_created' &&
    input.workResult.artifactReceiptRef !== null) ||
  (input.mode === 'deferred_until_response_closeout' &&
    input.workResult.status === 'response_closed' &&
    input.workResult.responseCloseoutRef !== null) ||
  (input.mode === 'manual_operator_review' &&
    input.manualApprovalRef !== null &&
    (input.workResult.status === 'succeeded' ||
      input.workResult.status === 'artifact_receipt_created' ||
      input.workResult.status === 'response_closed'))

const reasonRefsForInput = (
  input: OpenAgentsL402DeferredSettlementInput,
  status: OpenAgentsL402DeferredSettlementStatus,
): ReadonlyArray<string> => {
  const statusReason =
    status === 'allow'
      ? 'reason.l402_deferred_settlement.active_entitlement'
      : status === 'blocked'
        ? challengeIsIssued(input)
          ? 'reason.l402_deferred_settlement.blocked'
          : 'reason.l402_deferred_settlement.challenge_expired_or_cancelled'
        : status === 'payment_required'
          ? 'reason.l402_deferred_settlement.credential_not_valid'
          : status === 'retryable_failure'
            ? 'reason.l402_deferred_settlement.work_failed_before_charge'
            : status === 'settled'
              ? 'reason.l402_deferred_settlement.success_boundary_reached'
              : `reason.l402_deferred_settlement.pending_${input.mode}`

  return safeRefs([
    statusReason,
    ...input.metadataRefs,
  ])
}

const statusForInput = (
  input: OpenAgentsL402DeferredSettlementInput,
): OpenAgentsL402DeferredSettlementStatus =>
  existingSettlementMatches(input)
    ? 'settled'
    : activeEntitlementMatches(input)
      ? 'allow'
      : !challengeIsIssued(input)
        ? 'blocked'
        : !verificationIsValid(input.verification)
          ? 'payment_required'
          : input.workResult.status === 'failed'
            ? 'retryable_failure'
            : successBoundarySatisfied(input)
              ? 'settled'
              : 'settlement_pending'

const receiptRefForInput = (
  input: OpenAgentsL402DeferredSettlementInput,
  status: OpenAgentsL402DeferredSettlementStatus,
): string | null =>
  input.existingReceipt?.receiptRef ??
  (status === 'settled'
    ? safeRef(
      [
        'receipt',
        'l402_deferred',
        input.productId,
        input.buyerPaymentChallenge.challengeRef,
      ].join('.'),
    ) ?? 'receipt.l402_deferred.redacted'
    : null)

const entitlementRefForInput = (
  input: OpenAgentsL402DeferredSettlementInput,
  status: OpenAgentsL402DeferredSettlementStatus,
): string | null =>
  input.existingEntitlement?.entitlementRef ??
  (status === 'settled'
    ? safeRef(
      [
        'entitlement',
        'l402_deferred',
        input.productId,
        input.buyerPaymentChallenge.challengeRef,
      ].join('.'),
    ) ?? 'entitlement.l402_deferred.redacted'
    : null)

const attemptRefForInput = (
  input: OpenAgentsL402DeferredSettlementInput,
): string =>
  safeRef(
    [
      'attempt',
      'l402_deferred',
      input.productId,
      input.buyerPaymentChallenge.challengeRef,
      input.idempotencyKeyHash,
    ].join('.'),
  ) ?? 'attempt.l402_deferred.redacted'

const settlementRefForInput = (
  input: OpenAgentsL402DeferredSettlementInput,
): string =>
  safeRef(
    [
      'settlement',
      'l402_deferred',
      input.productId,
      input.buyerPaymentChallenge.challengeRef,
    ].join('.'),
  ) ?? 'settlement.l402_deferred.redacted'

const projectionForRecord = (
  record: OpenAgentsL402DeferredSettlementAttemptRecord,
  input: OpenAgentsL402DeferredSettlementInput,
): OpenAgentsL402DeferredSettlementProjection => {
  const credentialRef =
    input.audience === 'operator' ? record.credentialRef : null
  const actorRef =
    input.audience === 'operator' || input.audience === 'agent'
      ? nullableSafeRef(record.actorRef)
      : null

  return {
    actorRef,
    attemptRef: record.attemptRef,
    audience: input.audience,
    buyerPaymentChallenge:
      record.status === 'payment_required' || record.status === 'blocked'
        ? projectBuyerPaymentLedgerRecord(
          'challenge',
          input.buyerPaymentChallenge,
          input.audience,
        )
        : null,
    credentialConsumed: record.credentialConsumed,
    credentialRef,
    credentialReusable: record.credentialReusable,
    endpointRef: record.endpointRef,
    entitlement: input.existingEntitlement === null
      ? null
      : projectBuyerPaymentLedgerRecord(
        'entitlement',
        input.existingEntitlement,
        input.audience,
      ),
    failureRef: record.failureRef,
    productId: record.productId,
    reasonRefs: record.reasonRefs,
    receipt: input.existingReceipt === null
      ? null
      : projectBuyerPaymentLedgerRecord(
        'receipt',
        input.existingReceipt,
        input.audience,
      ),
    retryable: record.retryable,
    safeBody: {
      action: 'l402_deferred_settlement',
      challengeRef: input.buyerPaymentChallenge.challengeRef,
      credentialReusable: record.credentialReusable,
      entitlementRef: record.entitlementRef,
      receiptRef: record.receiptRef,
      settlementRef: record.settlementRef,
      status: record.status,
    },
    settlementRef: record.settlementRef,
    status: record.status,
    statusCode: statusCodeForStatus(record.status),
    surface: record.surface,
  }
}

const projectionIsSafe = (
  projection: OpenAgentsL402DeferredSettlementProjection,
): boolean =>
  scanForUnsafeSettlementMaterial(projection) === undefined

export const openAgentsL402DeferredSettlementHasPrivateMaterial = (
  value: unknown,
): boolean => scanForUnsafeSettlementMaterial(value) !== undefined

export const evaluateOpenAgentsL402DeferredSettlement = (
  input: OpenAgentsL402DeferredSettlementInput,
): OpenAgentsL402DeferredSettlementProjection => {
  assertSafeSettlementValue('OpenAgents L402 deferred settlement input', input)

  const status = statusForInput(input)
  const receiptRef = receiptRefForInput(input, status)
  const entitlementRef = entitlementRefForInput(input, status)
  const credentialRef = verificationCredentialRef(input.verification)
  const credentialConsumed = status === 'settled'
  const credentialReusable =
    status === 'retryable_failure' || status === 'settlement_pending'
  const record: OpenAgentsL402DeferredSettlementAttemptRecord = {
    actorRef: nullableSafeRef(input.actorRef) ?? 'actor.l402_deferred.redacted',
    attemptRef: attemptRefForInput(input),
    challengeRef: input.buyerPaymentChallenge.challengeRef,
    createdAt: input.nowIso,
    credentialConsumed,
    credentialRef,
    credentialReusable,
    endpointRef: nullableSafeRef(input.endpointRef) ??
      'endpoint.l402_deferred.redacted',
    entitlementRef,
    failureRef: status === 'retryable_failure'
      ? nullableSafeRef(input.workResult.failureRef) ??
        'failure.l402_deferred.retryable'
      : null,
    idempotencyKeyHash: input.idempotencyKeyHash,
    metadataRefs: safeRefs(input.metadataRefs),
    mode: input.mode,
    productId: nullableSafeRef(input.productId) ??
      'product.l402_deferred.redacted',
    reasonRefs: reasonRefsForInput(input, status),
    receiptRef,
    retryable: status === 'retryable_failure' && input.workResult.retryable,
    settlementRef: settlementRefForInput(input),
    status,
    surface: input.surface,
    updatedAt: input.nowIso,
    workResult: {
      artifactReceiptRef: nullableSafeRef(input.workResult.artifactReceiptRef),
      failureRef: nullableSafeRef(input.workResult.failureRef),
      responseCloseoutRef: nullableSafeRef(input.workResult.responseCloseoutRef),
      retryable: input.workResult.retryable,
      status: input.workResult.status,
    },
  }
  const projection = projectionForRecord(record, input)

  if (!projectionIsSafe(projection)) {
    throw new OpenAgentsL402DeferredSettlementUnsafe({
      reason:
        'OpenAgents L402 deferred settlement projection is not public-safe.',
    })
  }

  return projection
}
