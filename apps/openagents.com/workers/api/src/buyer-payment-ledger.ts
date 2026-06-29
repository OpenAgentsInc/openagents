import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  decodeUnknownWithSchema,
  parseJsonStringArray,
} from './json-boundary'
import {
  OpenAgentsPaidEndpointAsset,
  OpenAgentsPaidEndpointDenomination,
  OpenAgentsPaidEndpointMethod,
} from './paid-endpoint-product-catalog'
import {
  OpenAgentsPaymentPolicyAudience,
  OpenAgentsPaymentPolicySurface,
} from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const BuyerPaymentLedgerRecordKind = S.Literals([
  'challenge',
  'credit_debit',
  'entitlement',
  'receipt',
  'reconciliation_event',
  'redemption',
  'spend_limit',
])
export type BuyerPaymentLedgerRecordKind =
  typeof BuyerPaymentLedgerRecordKind.Type

export const BuyerPaymentChallengeStatus = S.Literals([
  'cancelled',
  'expired',
  'issued',
])
export type BuyerPaymentChallengeStatus =
  typeof BuyerPaymentChallengeStatus.Type

export const BuyerPaymentReceiptStatus = S.Literals(['issued', 'voided'])
export type BuyerPaymentReceiptStatus = typeof BuyerPaymentReceiptStatus.Type

export const BuyerPaymentEntitlementStatus = S.Literals([
  'active',
  'consumed',
  'expired',
  'revoked',
])
export type BuyerPaymentEntitlementStatus =
  typeof BuyerPaymentEntitlementStatus.Type

export const BuyerPaymentRedemptionStatus = S.Literals([
  'redeemed',
  'rejected',
  'replayed',
])
export type BuyerPaymentRedemptionStatus =
  typeof BuyerPaymentRedemptionStatus.Type

export const BuyerPaymentSpendLimitStatus = S.Literals([
  'active',
  'exhausted',
  'revoked',
])
export type BuyerPaymentSpendLimitStatus =
  typeof BuyerPaymentSpendLimitStatus.Type

export const BuyerPaymentCreditDebitStatus = S.Literals([
  'captured',
  'released',
  'reserved',
  'voided',
])
export type BuyerPaymentCreditDebitStatus =
  typeof BuyerPaymentCreditDebitStatus.Type

export const BuyerPaymentReconciliationStatus = S.Literals([
  'matched',
  'observed',
  'rejected',
  'replayed',
])
export type BuyerPaymentReconciliationStatus =
  typeof BuyerPaymentReconciliationStatus.Type

export const BuyerPaymentLedgerAmount = S.Struct({
  amountMinorUnits: S.Number,
  asset: OpenAgentsPaidEndpointAsset,
  denomination: OpenAgentsPaidEndpointDenomination,
})
export type BuyerPaymentLedgerAmount =
  typeof BuyerPaymentLedgerAmount.Type

export const BuyerPaymentChallengeRecord = S.Struct({
  actorRef: S.String,
  archivedAt: S.NullOr(S.String),
  challengeRef: S.String,
  createdAt: S.String,
  expiresAt: S.String,
  id: S.String,
  idempotencyKeyHash: S.String,
  metadataRefs: S.Array(S.String),
  method: OpenAgentsPaidEndpointMethod,
  ownerUserId: S.NullOr(S.String),
  path: S.String,
  price: BuyerPaymentLedgerAmount,
  productId: S.String,
  publicProjectionJson: S.String,
  requestBodyDigest: S.String,
  spendCap: BuyerPaymentLedgerAmount,
  status: BuyerPaymentChallengeStatus,
  surface: OpenAgentsPaymentPolicySurface,
})
export type BuyerPaymentChallengeRecord =
  typeof BuyerPaymentChallengeRecord.Type

export const BuyerPaymentReceiptRecord = S.Struct({
  actorRef: S.String,
  amount: BuyerPaymentLedgerAmount,
  archivedAt: S.NullOr(S.String),
  challengeRef: S.String,
  createdAt: S.String,
  entitlementRef: S.String,
  id: S.String,
  metadataRefs: S.Array(S.String),
  ownerUserId: S.NullOr(S.String),
  productId: S.String,
  publicProjectionJson: S.String,
  receiptRef: S.String,
  redactedPaymentRef: S.String,
  status: BuyerPaymentReceiptStatus,
  surface: OpenAgentsPaymentPolicySurface,
})
export type BuyerPaymentReceiptRecord = typeof BuyerPaymentReceiptRecord.Type

export const BuyerPaymentEntitlementRecord = S.Struct({
  actorRef: S.String,
  archivedAt: S.NullOr(S.String),
  challengeRef: S.String,
  consumedAt: S.NullOr(S.String),
  createdAt: S.String,
  entitlementRef: S.String,
  expiresAt: S.NullOr(S.String),
  id: S.String,
  ownerUserId: S.NullOr(S.String),
  productId: S.String,
  receiptRef: S.String,
  scopeRefs: S.Array(S.String),
  status: BuyerPaymentEntitlementStatus,
  surface: OpenAgentsPaymentPolicySurface,
})
export type BuyerPaymentEntitlementRecord =
  typeof BuyerPaymentEntitlementRecord.Type

export const BuyerPaymentRedemptionRecord = S.Struct({
  actorRef: S.String,
  archivedAt: S.NullOr(S.String),
  challengeRef: S.String,
  createdAt: S.String,
  entitlementRef: S.String,
  id: S.String,
  idempotencyKeyHash: S.String,
  metadataRefs: S.Array(S.String),
  proofRef: S.String,
  receiptRef: S.String,
  redemptionRef: S.String,
  replayed: S.Number,
  status: BuyerPaymentRedemptionStatus,
})
export type BuyerPaymentRedemptionRecord =
  typeof BuyerPaymentRedemptionRecord.Type

export const BuyerPaymentSpendLimitRecord = S.Struct({
  actorRef: S.String,
  amount: BuyerPaymentLedgerAmount,
  archivedAt: S.NullOr(S.String),
  createdAt: S.String,
  id: S.String,
  metadataRefs: S.Array(S.String),
  ownerUserId: S.NullOr(S.String),
  productId: S.NullOr(S.String),
  scopeRef: S.String,
  spendLimitRef: S.String,
  status: BuyerPaymentSpendLimitStatus,
  updatedAt: S.String,
  windowRef: S.String,
})
export type BuyerPaymentSpendLimitRecord =
  typeof BuyerPaymentSpendLimitRecord.Type

export const BuyerPaymentCreditDebitRecord = S.Struct({
  actorRef: S.String,
  amount: BuyerPaymentLedgerAmount,
  archivedAt: S.NullOr(S.String),
  billingLedgerEntryRef: S.NullOr(S.String),
  createdAt: S.String,
  debitRef: S.String,
  id: S.String,
  idempotencyKeyHash: S.String,
  metadataRefs: S.Array(S.String),
  ownerUserId: S.NullOr(S.String),
  productId: S.String,
  publicProjectionJson: S.String,
  receiptRef: S.NullOr(S.String),
  status: BuyerPaymentCreditDebitStatus,
})
export type BuyerPaymentCreditDebitRecord =
  typeof BuyerPaymentCreditDebitRecord.Type

export const BuyerPaymentReconciliationEventRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  challengeRef: S.NullOr(S.String),
  createdAt: S.String,
  eventRef: S.String,
  externalEventRef: S.String,
  id: S.String,
  idempotencyKeyHash: S.String,
  metadataRefs: S.Array(S.String),
  productId: S.NullOr(S.String),
  providerRef: S.String,
  publicProjectionJson: S.String,
  receiptRef: S.NullOr(S.String),
  resultRef: S.String,
  status: BuyerPaymentReconciliationStatus,
})
export type BuyerPaymentReconciliationEventRecord =
  typeof BuyerPaymentReconciliationEventRecord.Type

export type BuyerPaymentLedgerRecord =
  | BuyerPaymentChallengeRecord
  | BuyerPaymentCreditDebitRecord
  | BuyerPaymentEntitlementRecord
  | BuyerPaymentReceiptRecord
  | BuyerPaymentReconciliationEventRecord
  | BuyerPaymentRedemptionRecord
  | BuyerPaymentSpendLimitRecord

export const BuyerPaymentLedgerProjection = S.Struct({
  actorRef: S.NullOr(S.String),
  amount: S.NullOr(BuyerPaymentLedgerAmount),
  audience: OpenAgentsPaymentPolicyAudience,
  challengeRef: S.NullOr(S.String),
  entitlementRef: S.NullOr(S.String),
  metadataRefs: S.Array(S.String),
  operatorRefs: S.Array(S.String),
  ownerUserId: S.NullOr(S.String),
  productId: S.NullOr(S.String),
  publicProjectionJson: S.String,
  receiptRef: S.NullOr(S.String),
  recordKind: BuyerPaymentLedgerRecordKind,
  redactedPaymentRef: S.NullOr(S.String),
  status: S.String,
  surface: S.NullOr(OpenAgentsPaymentPolicySurface),
})
export type BuyerPaymentLedgerProjection =
  typeof BuyerPaymentLedgerProjection.Type

export class BuyerPaymentLedgerUnsafe extends S.TaggedErrorClass<BuyerPaymentLedgerUnsafe>()(
  'BuyerPaymentLedgerUnsafe',
  {
    reason: S.String,
  },
) {}

export class BuyerPaymentLedgerStorageError extends S.TaggedErrorClass<BuyerPaymentLedgerStorageError>()(
  'BuyerPaymentLedgerStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

const unsafeLedgerKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|cookie|customer[_-]?(email|name)|email[_-]?body|grant|invoice|mdk|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|source[_-]?archive|wallet|webhook)/i

const unsafeLedgerValuePattern =
  /(bearer\s+|callback[_-]?token|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github\.com\/[^:/]+\/private|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk_access_token|mnemonic|payment[_-]?proof|payment_hash=|payment_preimage=|preimage|provider[_-]?grant|provider[_-]?token|raw[_-]?invoice|raw[_-]?payment|raw[_-]?payload|raw[_-]?prompt|raw[_-]?runner|raw[_-]?run[_-]?log|secret|sk-[a-z0-9]|wallet|\S+@\S+|\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})/i

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const rawLedgerTimestampPattern =
  /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const ledgerTimestampKeys = new Set([
  'archivedAt',
  'consumedAt',
  'createdAt',
  'expiresAt',
  'updatedAt',
])

const scanForUnsafeLedgerMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    const key = path.at(-1)

    if (
      key !== undefined &&
      ledgerTimestampKeys.has(key) &&
      rawLedgerTimestampPattern.test(value)
    ) {
      return undefined
    }

    return containsProviderSecretMaterial(value) ||
      unsafeLedgerValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
      ? path.join('.') || '<root>'
      : undefined
  }

  if (Array.isArray(value)) {
    for (const [index, item] of value.entries()) {
      const unsafePath = scanForUnsafeLedgerMaterial(item, [
        ...path,
        String(index),
      ])

      if (unsafePath !== undefined) {
        return unsafePath
      }
    }

    return undefined
  }

  if (value === null || typeof value !== 'object') {
    return undefined
  }

  if (openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)) {
    return path.join('.') || '<root>'
  }

  for (const [key, item] of Object.entries(value)) {
    if (unsafeLedgerKeyPattern.test(key)) {
      return [...path, key].join('.')
    }

    const unsafePath = scanForUnsafeLedgerMaterial(item, [...path, key])

    if (unsafePath !== undefined) {
      return unsafePath
    }
  }

  return undefined
}

const safeRef = (value: string): string | undefined =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  scanForUnsafeLedgerMaterial(value) === undefined
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

const assertSafeLedgerValue = (label: string, value: unknown): void => {
  const unsafePath = scanForUnsafeLedgerMaterial(value)

  if (unsafePath !== undefined) {
    throw new BuyerPaymentLedgerUnsafe({
      reason: `${label} contains private or payment-secret material at ${unsafePath}.`,
    })
  }
}

const assertPositiveInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new BuyerPaymentLedgerUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

export const decodeBuyerPaymentLedgerAmount = (
  value: unknown,
): BuyerPaymentLedgerAmount => {
  assertSafeLedgerValue('Buyer payment amount', value)
  const amount = decodeUnknownWithSchema(BuyerPaymentLedgerAmount, value)
  assertPositiveInteger('amountMinorUnits', amount.amountMinorUnits)

  const expectedDenomination =
    amount.asset === 'usd'
      ? 'usd_cent'
      : amount.asset === 'bitcoin'
        ? 'bitcoin_millisatoshi'
        : 'credit'

  if (amount.denomination !== expectedDenomination) {
    throw new BuyerPaymentLedgerUnsafe({
      reason: `amount denomination must match ${amount.asset}.`,
    })
  }

  return amount
}

export const assertBuyerPaymentLedgerRecordSafe = (
  label: string,
  record: unknown,
): void => {
  assertSafeLedgerValue(label, record)
}

const jsonStringArray = (values: ReadonlyArray<string>): string =>
  JSON.stringify([...new Set(values)])

const safeJsonString = (json: string): string =>
  scanForUnsafeLedgerMaterial(json) === undefined ? json : '{}'

const storageError = (
  operation: string,
  error: unknown,
): BuyerPaymentLedgerStorageError =>
  new BuyerPaymentLedgerStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

type ChallengeRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  challenge_ref: string
  created_at: string
  expires_at: string
  id: string
  idempotency_key_hash: string
  metadata_refs_json: string
  method: typeof OpenAgentsPaidEndpointMethod.Type
  owner_user_id: string | null
  path: string
  price_amount_minor_units: number
  price_asset: typeof OpenAgentsPaidEndpointAsset.Type
  price_denomination: typeof OpenAgentsPaidEndpointDenomination.Type
  product_id: string
  public_projection_json: string
  request_body_digest: string
  spend_cap_amount_minor_units: number
  spend_cap_asset: typeof OpenAgentsPaidEndpointAsset.Type
  spend_cap_denomination: typeof OpenAgentsPaidEndpointDenomination.Type
  status: BuyerPaymentChallengeStatus
  surface: typeof OpenAgentsPaymentPolicySurface.Type
}>

type ReceiptRow = Readonly<{
  actor_ref: string
  amount_amount_minor_units: number
  amount_asset: typeof OpenAgentsPaidEndpointAsset.Type
  amount_denomination: typeof OpenAgentsPaidEndpointDenomination.Type
  archived_at: string | null
  challenge_ref: string
  created_at: string
  entitlement_ref: string
  id: string
  metadata_refs_json: string
  owner_user_id: string | null
  product_id: string
  public_projection_json: string
  receipt_ref: string
  redacted_payment_ref: string
  status: BuyerPaymentReceiptStatus
  surface: typeof OpenAgentsPaymentPolicySurface.Type
}>

type EntitlementRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  challenge_ref: string
  consumed_at: string | null
  created_at: string
  entitlement_ref: string
  expires_at: string | null
  id: string
  owner_user_id: string | null
  product_id: string
  receipt_ref: string
  scope_refs_json: string
  status: BuyerPaymentEntitlementStatus
  surface: typeof OpenAgentsPaymentPolicySurface.Type
}>

type RedemptionRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  challenge_ref: string
  created_at: string
  entitlement_ref: string
  id: string
  idempotency_key_hash: string
  metadata_refs_json: string
  proof_ref: string
  receipt_ref: string
  redemption_ref: string
  replayed: number
  status: BuyerPaymentRedemptionStatus
}>

type ReconciliationRow = Readonly<{
  archived_at: string | null
  challenge_ref: string | null
  created_at: string
  event_ref: string
  external_event_ref: string
  id: string
  idempotency_key_hash: string
  metadata_refs_json: string
  product_id: string | null
  provider_ref: string
  public_projection_json: string
  receipt_ref: string | null
  result_ref: string
  status: BuyerPaymentReconciliationStatus
}>

const amountFromRow = (
  asset: typeof OpenAgentsPaidEndpointAsset.Type,
  denomination: typeof OpenAgentsPaidEndpointDenomination.Type,
  amountMinorUnits: number,
): BuyerPaymentLedgerAmount => ({
  amountMinorUnits,
  asset,
  denomination,
})

export const buyerPaymentChallengeFromRow = (
  row: ChallengeRow,
): BuyerPaymentChallengeRecord => ({
  actorRef: row.actor_ref,
  archivedAt: row.archived_at,
  challengeRef: row.challenge_ref,
  createdAt: row.created_at,
  expiresAt: row.expires_at,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  metadataRefs: parseJsonStringArray(row.metadata_refs_json),
  method: row.method,
  ownerUserId: row.owner_user_id,
  path: row.path,
  price: amountFromRow(
    row.price_asset,
    row.price_denomination,
    row.price_amount_minor_units,
  ),
  productId: row.product_id,
  publicProjectionJson: row.public_projection_json,
  requestBodyDigest: row.request_body_digest,
  spendCap: amountFromRow(
    row.spend_cap_asset,
    row.spend_cap_denomination,
    row.spend_cap_amount_minor_units,
  ),
  status: row.status,
  surface: row.surface,
})

export const buyerPaymentReceiptFromRow = (
  row: ReceiptRow,
): BuyerPaymentReceiptRecord => ({
  actorRef: row.actor_ref,
  amount: amountFromRow(
    row.amount_asset,
    row.amount_denomination,
    row.amount_amount_minor_units,
  ),
  archivedAt: row.archived_at,
  challengeRef: row.challenge_ref,
  createdAt: row.created_at,
  entitlementRef: row.entitlement_ref,
  id: row.id,
  metadataRefs: parseJsonStringArray(row.metadata_refs_json),
  ownerUserId: row.owner_user_id,
  productId: row.product_id,
  publicProjectionJson: row.public_projection_json,
  receiptRef: row.receipt_ref,
  redactedPaymentRef: row.redacted_payment_ref,
  status: row.status,
  surface: row.surface,
})

export const buyerPaymentEntitlementFromRow = (
  row: EntitlementRow,
): BuyerPaymentEntitlementRecord => ({
  actorRef: row.actor_ref,
  archivedAt: row.archived_at,
  challengeRef: row.challenge_ref,
  consumedAt: row.consumed_at,
  createdAt: row.created_at,
  entitlementRef: row.entitlement_ref,
  expiresAt: row.expires_at,
  id: row.id,
  ownerUserId: row.owner_user_id,
  productId: row.product_id,
  receiptRef: row.receipt_ref,
  scopeRefs: parseJsonStringArray(row.scope_refs_json),
  status: row.status,
  surface: row.surface,
})

export const buyerPaymentRedemptionFromRow = (
  row: RedemptionRow,
): BuyerPaymentRedemptionRecord => ({
  actorRef: row.actor_ref,
  archivedAt: row.archived_at,
  challengeRef: row.challenge_ref,
  createdAt: row.created_at,
  entitlementRef: row.entitlement_ref,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  metadataRefs: parseJsonStringArray(row.metadata_refs_json),
  proofRef: row.proof_ref,
  receiptRef: row.receipt_ref,
  redemptionRef: row.redemption_ref,
  replayed: row.replayed,
  status: row.status,
})

export const buyerPaymentReconciliationEventFromRow = (
  row: ReconciliationRow,
): BuyerPaymentReconciliationEventRecord => ({
  archivedAt: row.archived_at,
  challengeRef: row.challenge_ref,
  createdAt: row.created_at,
  eventRef: row.event_ref,
  externalEventRef: row.external_event_ref,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  metadataRefs: parseJsonStringArray(row.metadata_refs_json),
  productId: row.product_id,
  providerRef: row.provider_ref,
  publicProjectionJson: row.public_projection_json,
  receiptRef: row.receipt_ref,
  resultRef: row.result_ref,
  status: row.status,
})

export const projectBuyerPaymentLedgerRecord = (
  recordKind: BuyerPaymentLedgerRecordKind,
  record: BuyerPaymentLedgerRecord,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): BuyerPaymentLedgerProjection => {
  const operator = audience === 'operator'
  const customerOrAgent = audience === 'customer' || audience === 'agent'
  const amount =
    'amount' in record
      ? record.amount
      : 'price' in record
        ? record.price
        : null
  const challengeRef =
    'challengeRef' in record ? nullableSafeRef(record.challengeRef) : null
  const receiptRef =
    'receiptRef' in record ? nullableSafeRef(record.receiptRef) : null
  const entitlementRef =
    'entitlementRef' in record ? nullableSafeRef(record.entitlementRef) : null
  const metadataRefs =
    'metadataRefs' in record ? safeRefs(record.metadataRefs) : []
  const actorRef = 'actorRef' in record ? nullableSafeRef(record.actorRef) : null
  const ownerUserId =
    'ownerUserId' in record ? nullableSafeRef(record.ownerUserId) : null
  const productId =
    'productId' in record ? nullableSafeRef(record.productId) : null
  const publicProjectionJson =
    'publicProjectionJson' in record
      ? safeJsonString(record.publicProjectionJson)
      : '{}'
  const surface = 'surface' in record ? record.surface : null
  const operatorRefs = operator
    ? safeRefs([
        ...metadataRefs,
        ...('providerRef' in record ? [record.providerRef] : []),
        ...('externalEventRef' in record ? [record.externalEventRef] : []),
        ...('billingLedgerEntryRef' in record &&
        record.billingLedgerEntryRef !== null
          ? [record.billingLedgerEntryRef]
          : []),
      ])
    : []

  return {
    actorRef:
      operator || customerOrAgent ? actorRef : null,
    amount,
    audience,
    challengeRef,
    entitlementRef,
    metadataRefs: operator ? metadataRefs : [],
    operatorRefs,
    ownerUserId: operator ? ownerUserId : null,
    productId,
    publicProjectionJson,
    receiptRef,
    recordKind,
    redactedPaymentRef:
      'redactedPaymentRef' in record && (operator || customerOrAgent)
        ? safeRef(record.redactedPaymentRef) ?? null
        : null,
    status: record.status,
    surface,
  }
}

export const buyerPaymentLedgerProjectionHasPrivateMaterial = (
  projection: BuyerPaymentLedgerProjection,
): boolean => scanForUnsafeLedgerMaterial(projection) !== undefined

export type BuyerPaymentLedgerStore = Readonly<{
  createChallenge: (record: BuyerPaymentChallengeRecord) => Promise<void>
  createCreditDebit: (record: BuyerPaymentCreditDebitRecord) => Promise<void>
  createReceiptEntitlementBundle: (input: {
    entitlement: BuyerPaymentEntitlementRecord
    receipt: BuyerPaymentReceiptRecord
  }) => Promise<void>
  createReconciliationEvent: (
    record: BuyerPaymentReconciliationEventRecord,
  ) => Promise<void>
  createRedemptionBundle: (input: {
    entitlement: BuyerPaymentEntitlementRecord
    receipt: BuyerPaymentReceiptRecord
    redemption: BuyerPaymentRedemptionRecord
  }) => Promise<void>
  createSpendLimit: (record: BuyerPaymentSpendLimitRecord) => Promise<void>
  readChallengeByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<BuyerPaymentChallengeRecord | undefined>
  readEntitlementByRef: (
    entitlementRef: string,
  ) => Promise<BuyerPaymentEntitlementRecord | undefined>
  readReceiptByRef: (
    receiptRef: string,
  ) => Promise<BuyerPaymentReceiptRecord | undefined>
  readReconciliationEventByReceiptRef: (
    receiptRef: string,
  ) => Promise<BuyerPaymentReconciliationEventRecord | undefined>
  readReconciliationEventByProviderEvent: (
    providerRef: string,
    externalEventRef: string,
  ) => Promise<BuyerPaymentReconciliationEventRecord | undefined>
  readRedemptionByChallengeRef: (
    challengeRef: string,
  ) => Promise<BuyerPaymentRedemptionRecord | undefined>
}>

export const makeD1BuyerPaymentLedgerStore = (
  db: D1Database,
): BuyerPaymentLedgerStore => ({
  createChallenge: async record => {
    assertBuyerPaymentLedgerRecordSafe('Buyer payment challenge', record)
    decodeBuyerPaymentLedgerAmount(record.price)
    decodeBuyerPaymentLedgerAmount(record.spendCap)

    try {
      await db
        .prepare(
          `INSERT OR IGNORE INTO buyer_payment_challenges
           (id, challenge_ref, idempotency_key_hash, actor_ref, owner_user_id,
            product_id, surface, method, path, request_body_digest,
            price_asset, price_denomination, price_amount_minor_units,
            spend_cap_asset, spend_cap_denomination,
            spend_cap_amount_minor_units, status, expires_at,
            metadata_refs_json, public_projection_json, created_at, archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.id,
          record.challengeRef,
          record.idempotencyKeyHash,
          record.actorRef,
          record.ownerUserId,
          record.productId,
          record.surface,
          record.method,
          record.path,
          record.requestBodyDigest,
          record.price.asset,
          record.price.denomination,
          record.price.amountMinorUnits,
          record.spendCap.asset,
          record.spendCap.denomination,
          record.spendCap.amountMinorUnits,
          record.status,
          record.expiresAt,
          jsonStringArray(record.metadataRefs),
          record.publicProjectionJson,
          record.createdAt,
          record.archivedAt,
        )
        .run()
    } catch (error) {
      throw storageError('buyerPaymentLedger.createChallenge', error)
    }
  },
  createCreditDebit: async record => {
    assertBuyerPaymentLedgerRecordSafe('Buyer payment credit debit', record)
    decodeBuyerPaymentLedgerAmount(record.amount)

    try {
      await db
        .prepare(
          `INSERT OR IGNORE INTO buyer_payment_credit_debits
           (id, debit_ref, idempotency_key_hash, actor_ref, owner_user_id,
            product_id, amount_asset, amount_denomination,
            amount_minor_units, billing_ledger_entry_ref, receipt_ref, status,
            metadata_refs_json, public_projection_json, created_at, archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.id,
          record.debitRef,
          record.idempotencyKeyHash,
          record.actorRef,
          record.ownerUserId,
          record.productId,
          record.amount.asset,
          record.amount.denomination,
          record.amount.amountMinorUnits,
          record.billingLedgerEntryRef,
          record.receiptRef,
          record.status,
          jsonStringArray(record.metadataRefs),
          record.publicProjectionJson,
          record.createdAt,
          record.archivedAt,
        )
        .run()
    } catch (error) {
      throw storageError('buyerPaymentLedger.createCreditDebit', error)
    }
  },
  createReceiptEntitlementBundle: async input => {
    assertBuyerPaymentLedgerRecordSafe(
      'Buyer payment receipt entitlement bundle',
      input,
    )
    decodeBuyerPaymentLedgerAmount(input.receipt.amount)

    try {
      await db.batch([
        db
          .prepare(
            `INSERT OR IGNORE INTO buyer_payment_receipts
             (id, receipt_ref, challenge_ref, actor_ref, owner_user_id,
              product_id, surface, amount_asset, amount_denomination,
              amount_minor_units, entitlement_ref, redacted_payment_ref,
              status, metadata_refs_json, public_projection_json, created_at,
              archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.receipt.id,
            input.receipt.receiptRef,
            input.receipt.challengeRef,
            input.receipt.actorRef,
            input.receipt.ownerUserId,
            input.receipt.productId,
            input.receipt.surface,
            input.receipt.amount.asset,
            input.receipt.amount.denomination,
            input.receipt.amount.amountMinorUnits,
            input.receipt.entitlementRef,
            input.receipt.redactedPaymentRef,
            input.receipt.status,
            jsonStringArray(input.receipt.metadataRefs),
            input.receipt.publicProjectionJson,
            input.receipt.createdAt,
            input.receipt.archivedAt,
          ),
        db
          .prepare(
            `INSERT OR IGNORE INTO buyer_payment_entitlements
             (id, entitlement_ref, challenge_ref, receipt_ref, actor_ref,
              owner_user_id, product_id, surface, scope_refs_json, status,
              expires_at, created_at, consumed_at, archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.entitlement.id,
            input.entitlement.entitlementRef,
            input.entitlement.challengeRef,
            input.entitlement.receiptRef,
            input.entitlement.actorRef,
            input.entitlement.ownerUserId,
            input.entitlement.productId,
            input.entitlement.surface,
            jsonStringArray(input.entitlement.scopeRefs),
            input.entitlement.status,
            input.entitlement.expiresAt,
            input.entitlement.createdAt,
            input.entitlement.consumedAt,
            input.entitlement.archivedAt,
          ),
      ])
    } catch (error) {
      throw storageError(
        'buyerPaymentLedger.createReceiptEntitlementBundle',
        error,
      )
    }
  },
  createReconciliationEvent: async record => {
    assertBuyerPaymentLedgerRecordSafe(
      'Buyer payment reconciliation event',
      record,
    )

    try {
      await db
        .prepare(
          `INSERT OR IGNORE INTO buyer_payment_reconciliation_events
           (id, event_ref, idempotency_key_hash, provider_ref,
            external_event_ref, challenge_ref, receipt_ref, product_id, status,
            result_ref, metadata_refs_json, public_projection_json, created_at,
            archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.id,
          record.eventRef,
          record.idempotencyKeyHash,
          record.providerRef,
          record.externalEventRef,
          record.challengeRef,
          record.receiptRef,
          record.productId,
          record.status,
          record.resultRef,
          jsonStringArray(record.metadataRefs),
          record.publicProjectionJson,
          record.createdAt,
          record.archivedAt,
        )
        .run()
    } catch (error) {
      throw storageError('buyerPaymentLedger.createReconciliationEvent', error)
    }
  },
  createRedemptionBundle: async input => {
    assertBuyerPaymentLedgerRecordSafe(
      'Buyer payment redemption bundle',
      input,
    )
    decodeBuyerPaymentLedgerAmount(input.receipt.amount)

    try {
      await db.batch([
        db
          .prepare(
            `INSERT OR IGNORE INTO buyer_payment_receipts
             (id, receipt_ref, challenge_ref, actor_ref, owner_user_id,
              product_id, surface, amount_asset, amount_denomination,
              amount_minor_units, entitlement_ref, redacted_payment_ref,
              status, metadata_refs_json, public_projection_json, created_at,
              archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.receipt.id,
            input.receipt.receiptRef,
            input.receipt.challengeRef,
            input.receipt.actorRef,
            input.receipt.ownerUserId,
            input.receipt.productId,
            input.receipt.surface,
            input.receipt.amount.asset,
            input.receipt.amount.denomination,
            input.receipt.amount.amountMinorUnits,
            input.receipt.entitlementRef,
            input.receipt.redactedPaymentRef,
            input.receipt.status,
            jsonStringArray(input.receipt.metadataRefs),
            input.receipt.publicProjectionJson,
            input.receipt.createdAt,
            input.receipt.archivedAt,
          ),
        db
          .prepare(
            `INSERT OR IGNORE INTO buyer_payment_entitlements
             (id, entitlement_ref, challenge_ref, receipt_ref, actor_ref,
              owner_user_id, product_id, surface, scope_refs_json, status,
              expires_at, created_at, consumed_at, archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.entitlement.id,
            input.entitlement.entitlementRef,
            input.entitlement.challengeRef,
            input.entitlement.receiptRef,
            input.entitlement.actorRef,
            input.entitlement.ownerUserId,
            input.entitlement.productId,
            input.entitlement.surface,
            jsonStringArray(input.entitlement.scopeRefs),
            input.entitlement.status,
            input.entitlement.expiresAt,
            input.entitlement.createdAt,
            input.entitlement.consumedAt,
            input.entitlement.archivedAt,
          ),
        db
          .prepare(
            `INSERT OR IGNORE INTO buyer_payment_redemptions
             (id, redemption_ref, idempotency_key_hash, challenge_ref,
              actor_ref, proof_ref, entitlement_ref, receipt_ref, status,
              replayed, metadata_refs_json, public_projection_json, created_at,
              archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            input.redemption.id,
            input.redemption.redemptionRef,
            input.redemption.idempotencyKeyHash,
            input.redemption.challengeRef,
            input.redemption.actorRef,
            input.redemption.proofRef,
            input.redemption.entitlementRef,
            input.redemption.receiptRef,
            input.redemption.status,
            input.redemption.replayed,
            jsonStringArray(input.redemption.metadataRefs),
            '{}',
            input.redemption.createdAt,
            input.redemption.archivedAt,
          ),
      ])
    } catch (error) {
      throw storageError('buyerPaymentLedger.createRedemptionBundle', error)
    }
  },
  createSpendLimit: async record => {
    assertBuyerPaymentLedgerRecordSafe('Buyer payment spend limit', record)
    decodeBuyerPaymentLedgerAmount(record.amount)

    try {
      await db
        .prepare(
          `INSERT OR IGNORE INTO buyer_payment_spend_limits
           (id, spend_limit_ref, actor_ref, owner_user_id, product_id,
            scope_ref, window_ref, amount_asset, amount_denomination,
            amount_minor_units, status, metadata_refs_json, created_at,
            updated_at, archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          record.id,
          record.spendLimitRef,
          record.actorRef,
          record.ownerUserId,
          record.productId,
          record.scopeRef,
          record.windowRef,
          record.amount.asset,
          record.amount.denomination,
          record.amount.amountMinorUnits,
          record.status,
          jsonStringArray(record.metadataRefs),
          record.createdAt,
          record.updatedAt,
          record.archivedAt,
        )
        .run()
    } catch (error) {
      throw storageError('buyerPaymentLedger.createSpendLimit', error)
    }
  },
  readChallengeByIdempotencyKeyHash: async idempotencyKeyHash => {
    try {
      const row = await db
        .prepare(
          `SELECT *
             FROM buyer_payment_challenges
            WHERE idempotency_key_hash = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(idempotencyKeyHash)
        .first<ChallengeRow>()

      return row === null ? undefined : buyerPaymentChallengeFromRow(row)
    } catch (error) {
      throw storageError(
        'buyerPaymentLedger.readChallengeByIdempotencyKeyHash',
        error,
      )
    }
  },
  readEntitlementByRef: async entitlementRef => {
    try {
      const row = await db
        .prepare(
          `SELECT *
             FROM buyer_payment_entitlements
            WHERE entitlement_ref = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(entitlementRef)
        .first<EntitlementRow>()

      return row === null ? undefined : buyerPaymentEntitlementFromRow(row)
    } catch (error) {
      throw storageError('buyerPaymentLedger.readEntitlementByRef', error)
    }
  },
  readReceiptByRef: async receiptRef => {
    try {
      const row = await db
        .prepare(
          `SELECT *
             FROM buyer_payment_receipts
            WHERE receipt_ref = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(receiptRef)
        .first<ReceiptRow>()

      return row === null ? undefined : buyerPaymentReceiptFromRow(row)
    } catch (error) {
      throw storageError('buyerPaymentLedger.readReceiptByRef', error)
    }
  },
  readReconciliationEventByReceiptRef: async receiptRef => {
    try {
      const row = await db
        .prepare(
          `SELECT *
             FROM buyer_payment_reconciliation_events
            WHERE receipt_ref = ?
              AND archived_at IS NULL
            ORDER BY created_at DESC
            LIMIT 1`,
        )
        .bind(receiptRef)
        .first<ReconciliationRow>()

      return row === null
        ? undefined
        : buyerPaymentReconciliationEventFromRow(row)
    } catch (error) {
      throw storageError(
        'buyerPaymentLedger.readReconciliationEventByReceiptRef',
        error,
      )
    }
  },
  readReconciliationEventByProviderEvent: async (
    providerRef,
    externalEventRef,
  ) => {
    try {
      const row = await db
        .prepare(
          `SELECT *
             FROM buyer_payment_reconciliation_events
            WHERE provider_ref = ?
              AND external_event_ref = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(providerRef, externalEventRef)
        .first<ReconciliationRow>()

      return row === null
        ? undefined
        : buyerPaymentReconciliationEventFromRow(row)
    } catch (error) {
      throw storageError(
        'buyerPaymentLedger.readReconciliationEventByProviderEvent',
        error,
      )
    }
  },
  readRedemptionByChallengeRef: async challengeRef => {
    try {
      const row = await db
        .prepare(
          `SELECT *
             FROM buyer_payment_redemptions
            WHERE challenge_ref = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(challengeRef)
        .first<RedemptionRow>()

      return row === null ? undefined : buyerPaymentRedemptionFromRow(row)
    } catch (error) {
      throw storageError('buyerPaymentLedger.readRedemptionByChallengeRef', error)
    }
  },
})
