import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { decodeUnknownWithSchema, parseJsonStringArray } from './json-boundary'
import {
  OpenAgentsPaidEndpointAsset,
  OpenAgentsPaidEndpointDenomination,
} from './paid-endpoint-product-catalog'
import { OpenAgentsPaymentPolicyAudience } from './payment-limit-policy'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'
import {
  mirrorTreasuryRows,
  treasuryAuthorityDb,
  type TreasuryDatabase,
} from './treasury-domain-store'

export const NexusTreasuryPayoutAdapterKind = S.Literals([
  'hosted_mdk',
  'legacy_nexus_import',
  'mdk_agent_wallet',
  'simulation',
  'spark_treasury',
])
export type NexusTreasuryPayoutAdapterKind =
  typeof NexusTreasuryPayoutAdapterKind.Type

export const NexusTreasuryPayoutSourceKind = S.Literals([
  'accepted_work',
  'forum_reward',
  'operator_test',
  'pylon_marketplace_assignment',
])
export type NexusTreasuryPayoutSourceKind =
  typeof NexusTreasuryPayoutSourceKind.Type

export const NexusTreasuryPayoutIntentStatus = S.Literals([
  'approved',
  'cancelled',
  'dispatched',
  'failed',
  'proposed',
  'rejected',
  'settled',
])
export type NexusTreasuryPayoutIntentStatus =
  typeof NexusTreasuryPayoutIntentStatus.Type

export const NexusTreasuryPayoutAttemptStatus = S.Literals([
  'confirmed',
  'dispatched',
  'failed',
  'pending',
  'rejected',
  'replayed',
])
export type NexusTreasuryPayoutAttemptStatus =
  typeof NexusTreasuryPayoutAttemptStatus.Type

export const NexusTreasuryPayoutReconciliationStatus = S.Literals([
  'matched',
  'observed',
  'rejected',
  'replayed',
])
export type NexusTreasuryPayoutReconciliationStatus =
  typeof NexusTreasuryPayoutReconciliationStatus.Type

export const NexusPayoutTargetApprovalStatus = S.Literals([
  'active',
  'expired',
  'rejected',
  'revoked',
])
export type NexusPayoutTargetApprovalStatus =
  typeof NexusPayoutTargetApprovalStatus.Type

export const NexusPaymentAuthorityReceiptKind = S.Literals([
  'attempt_recorded',
  'confirmation_recorded',
  'dispatch_recorded',
  'intent_created',
  'pause_recorded',
  'policy_rejected',
  'settlement_recorded',
  'verification_recorded',
])
export type NexusPaymentAuthorityReceiptKind =
  typeof NexusPaymentAuthorityReceiptKind.Type

export const NexusReleaseGateKind = S.Literals([
  'artanis_real_assignment',
  'artanis_simulated_assignment',
  'mdk_adapter',
  'operator_dashboard',
  'public_receipt',
  'pylon_api',
  'pylon_v02_release',
  'simulation_adapter',
])
export type NexusReleaseGateKind = typeof NexusReleaseGateKind.Type

export const NexusReleaseGateStatus = S.Literals([
  'blocked',
  'failed',
  'passed',
  'pending',
])
export type NexusReleaseGateStatus = typeof NexusReleaseGateStatus.Type

export const NexusTreasuryPayoutAmount = S.Struct({
  amountMinorUnits: S.Number,
  asset: OpenAgentsPaidEndpointAsset,
  denomination: OpenAgentsPaidEndpointDenomination,
})
export type NexusTreasuryPayoutAmount = typeof NexusTreasuryPayoutAmount.Type

export const NexusPayoutTargetApprovalRecord = S.Struct({
  agentRef: S.NullOr(S.String),
  approvalPolicyRef: S.String,
  approvalRef: S.String,
  approvedByRef: S.String,
  archivedAt: S.NullOr(S.String),
  createdAt: S.String,
  expiresAt: S.NullOr(S.String),
  id: S.String,
  idempotencyKeyHash: S.String,
  ownerUserId: S.NullOr(S.String),
  publicProjectionJson: S.String,
  payoutTargetRef: S.String,
  pylonRef: S.NullOr(S.String),
  redactedDestinationRef: S.String,
  scopeRefs: S.Array(S.String),
  status: NexusPayoutTargetApprovalStatus,
  updatedAt: S.String,
})
export type NexusPayoutTargetApprovalRecord =
  typeof NexusPayoutTargetApprovalRecord.Type

export const NexusTreasuryPayoutIntentRecord = S.Struct({
  acceptedWorkRefs: S.Array(S.String),
  actorRef: S.String,
  adapterKind: NexusTreasuryPayoutAdapterKind,
  amount: NexusTreasuryPayoutAmount,
  archivedAt: S.NullOr(S.String),
  artanisDispatchRef: S.NullOr(S.String),
  assignmentRef: S.NullOr(S.String),
  buyerPaymentRef: S.NullOr(S.String),
  createdAt: S.String,
  id: S.String,
  idempotencyKeyHash: S.String,
  metadataRefs: S.Array(S.String),
  ownerUserId: S.NullOr(S.String),
  payoutIntentRef: S.String,
  payoutTargetApprovalRef: S.NullOr(S.String),
  payoutTargetRef: S.String,
  policySnapshotRef: S.String,
  publicProjectionJson: S.String,
  pylonJobRef: S.NullOr(S.String),
  sourceKind: NexusTreasuryPayoutSourceKind,
  spendCap: NexusTreasuryPayoutAmount,
  status: NexusTreasuryPayoutIntentStatus,
  updatedAt: S.String,
})
export type NexusTreasuryPayoutIntentRecord =
  typeof NexusTreasuryPayoutIntentRecord.Type

export const NexusTreasuryPayoutAttemptRecord = S.Struct({
  adapterAttemptRef: S.String,
  adapterKind: NexusTreasuryPayoutAdapterKind,
  amount: NexusTreasuryPayoutAmount,
  archivedAt: S.NullOr(S.String),
  createdAt: S.String,
  id: S.String,
  idempotencyKeyHash: S.String,
  metadataRefs: S.Array(S.String),
  payoutAttemptRef: S.String,
  payoutIntentRef: S.String,
  publicProjectionJson: S.String,
  redactedDestinationRef: S.String,
  redactedPaymentRef: S.NullOr(S.String),
  status: NexusTreasuryPayoutAttemptStatus,
  updatedAt: S.String,
})
export type NexusTreasuryPayoutAttemptRecord =
  typeof NexusTreasuryPayoutAttemptRecord.Type

export const NexusTreasuryPayoutReconciliationEventRecord = S.Struct({
  adapterKind: NexusTreasuryPayoutAdapterKind,
  archivedAt: S.NullOr(S.String),
  createdAt: S.String,
  eventRef: S.String,
  externalEventRef: S.String,
  id: S.String,
  idempotencyKeyHash: S.String,
  metadataRefs: S.Array(S.String),
  payoutAttemptRef: S.NullOr(S.String),
  payoutIntentRef: S.NullOr(S.String),
  providerRef: S.String,
  publicProjectionJson: S.String,
  resultRef: S.String,
  status: NexusTreasuryPayoutReconciliationStatus,
})
export type NexusTreasuryPayoutReconciliationEventRecord =
  typeof NexusTreasuryPayoutReconciliationEventRecord.Type

export const NexusPaymentAuthorityReceiptRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  audience: OpenAgentsPaymentPolicyAudience,
  createdAt: S.String,
  eventRef: S.NullOr(S.String),
  id: S.String,
  metadataRefs: S.Array(S.String),
  payoutAttemptRef: S.NullOr(S.String),
  payoutIntentRef: S.String,
  publicProjectionJson: S.String,
  receiptKind: NexusPaymentAuthorityReceiptKind,
  receiptRef: S.String,
})
export type NexusPaymentAuthorityReceiptRecord =
  typeof NexusPaymentAuthorityReceiptRecord.Type

export const NexusReleaseGateRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  blockerRefs: S.Array(S.String),
  createdAt: S.String,
  evidenceRefs: S.Array(S.String),
  gateKind: NexusReleaseGateKind,
  gateRef: S.String,
  id: S.String,
  idempotencyKeyHash: S.String,
  publicProjectionJson: S.String,
  status: NexusReleaseGateStatus,
  updatedAt: S.String,
})
export type NexusReleaseGateRecord = typeof NexusReleaseGateRecord.Type

export const NexusTreasuryPayoutLedgerRecordKind = S.Literals([
  'attempt',
  'intent',
  'payout_target_approval',
  'receipt',
  'reconciliation_event',
  'release_gate',
])
export type NexusTreasuryPayoutLedgerRecordKind =
  typeof NexusTreasuryPayoutLedgerRecordKind.Type

export type NexusTreasuryPayoutLedgerRecord =
  | NexusPaymentAuthorityReceiptRecord
  | NexusPayoutTargetApprovalRecord
  | NexusReleaseGateRecord
  | NexusTreasuryPayoutAttemptRecord
  | NexusTreasuryPayoutIntentRecord
  | NexusTreasuryPayoutReconciliationEventRecord

export const NexusTreasuryPayoutLedgerProjection = S.Struct({
  adapterKind: S.NullOr(NexusTreasuryPayoutAdapterKind),
  amount: S.NullOr(NexusTreasuryPayoutAmount),
  assignmentRef: S.NullOr(S.String),
  audience: OpenAgentsPaymentPolicyAudience,
  metadataRefs: S.Array(S.String),
  operatorRefs: S.Array(S.String),
  ownerUserId: S.NullOr(S.String),
  payoutAttemptRef: S.NullOr(S.String),
  payoutIntentRef: S.NullOr(S.String),
  payoutTargetApprovalRef: S.NullOr(S.String),
  payoutTargetRef: S.NullOr(S.String),
  publicProjectionJson: S.String,
  receiptRef: S.NullOr(S.String),
  recordKind: NexusTreasuryPayoutLedgerRecordKind,
  redactedDestinationRef: S.NullOr(S.String),
  redactedPaymentRef: S.NullOr(S.String),
  status: S.String,
})
export type NexusTreasuryPayoutLedgerProjection =
  typeof NexusTreasuryPayoutLedgerProjection.Type

export class NexusTreasuryPayoutLedgerUnsafe extends S.TaggedErrorClass<NexusTreasuryPayoutLedgerUnsafe>()(
  'NexusTreasuryPayoutLedgerUnsafe',
  {
    reason: S.String,
  },
) {}

export class NexusTreasuryPayoutLedgerStorageError extends S.TaggedErrorClass<NexusTreasuryPayoutLedgerStorageError>()(
  'NexusTreasuryPayoutLedgerStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const idempotencyKeyHashPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{7,180}$/
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const timestampKeys = new Set([
  'archivedAt',
  'createdAt',
  'expiresAt',
  'updatedAt',
])
const unsafeKeyPattern =
  /(access[_-]?token|bearer|callback[_-]?token|cookie|customer[_-]?(email|name)|email[_-]?body|full[_-]?destination|grant|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|source[_-]?archive|wallet[_-]?(config|mnemonic|secret|state)|webhook)/i
const unsafeValuePattern =
  /(bearer\s+|checkout_id=|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)=|preimage|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log)|secret|sk-[a-z0-9]|\S+@\S+|wallet[_-]?(config|mnemonic|secret|state))/i

const scanForUnsafeMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    const key = path.at(-1)

    if (
      key !== undefined &&
      timestampKeys.has(key) &&
      rawTimestampPattern.test(value)
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
    for (const [index, item] of value.entries()) {
      const unsafePath = scanForUnsafeMaterial(item, [...path, String(index)])

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
    if (unsafeKeyPattern.test(key)) {
      return [...path, key].join('.')
    }

    const unsafePath = scanForUnsafeMaterial(item, [...path, key])

    if (unsafePath !== undefined) {
      return unsafePath
    }
  }

  return undefined
}

const assertSafeValue = (label: string, value: unknown): void => {
  const unsafePath = scanForUnsafeMaterial(value)

  if (unsafePath !== undefined) {
    throw new NexusTreasuryPayoutLedgerUnsafe({
      reason: `${label} contains private payment or wallet material at ${unsafePath}.`,
    })
  }
}

const safeRef = (value: string): string | undefined =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  scanForUnsafeMaterial(value) === undefined
    ? value
    : undefined

const safeRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs)]
    .map(ref => safeRef(ref))
    .filter((ref): ref is string => ref !== undefined)

const nullableSafeRef = (value: string | null | undefined): string | null =>
  value === null || value === undefined ? null : (safeRef(value) ?? null)

const jsonStringArray = (values: ReadonlyArray<string>): string =>
  JSON.stringify([...new Set(values)])

const safeJsonString = (json: string): string =>
  scanForUnsafeMaterial(json) === undefined ? json : '{}'

const assertIdempotencyKeyHash = (label: string, value: string): void => {
  if (
    !idempotencyKeyHashPattern.test(value) ||
    scanForUnsafeMaterial(value) !== undefined
  ) {
    throw new NexusTreasuryPayoutLedgerUnsafe({
      reason: `${label} must be a stable redacted idempotency key hash.`,
    })
  }
}

const assertSafeRefRequired = (label: string, value: string): void => {
  if (safeRef(value) === undefined) {
    throw new NexusTreasuryPayoutLedgerUnsafe({
      reason: `${label} must be a stable redacted reference.`,
    })
  }
}

const assertNullableSafeRefRequired = (
  label: string,
  value: string | null,
): void => {
  if (value === null || safeRef(value) === undefined) {
    throw new NexusTreasuryPayoutLedgerUnsafe({
      reason: `${label} is required and must be a stable redacted reference.`,
    })
  }
}

const assertPositiveInteger = (label: string, value: number): void => {
  if (!Number.isInteger(value) || value < 0) {
    throw new NexusTreasuryPayoutLedgerUnsafe({
      reason: `${label} must be a non-negative integer.`,
    })
  }
}

export const decodeNexusTreasuryPayoutAmount = (
  value: unknown,
): NexusTreasuryPayoutAmount => {
  assertSafeValue('Nexus treasury payout amount', value)
  const amount = decodeUnknownWithSchema(NexusTreasuryPayoutAmount, value)
  assertPositiveInteger('amountMinorUnits', amount.amountMinorUnits)

  const expectedDenomination =
    amount.asset === 'usd'
      ? 'usd_cent'
      : amount.asset === 'bitcoin'
        ? 'bitcoin_millisatoshi'
        : 'credit'

  if (amount.denomination !== expectedDenomination) {
    throw new NexusTreasuryPayoutLedgerUnsafe({
      reason: `amount denomination must match ${amount.asset}.`,
    })
  }

  return amount
}

const amountWithinCap = (
  amount: NexusTreasuryPayoutAmount,
  spendCap: NexusTreasuryPayoutAmount,
): boolean =>
  amount.asset === spendCap.asset &&
  amount.denomination === spendCap.denomination &&
  amount.amountMinorUnits <= spendCap.amountMinorUnits

export const assertNexusTreasuryPayoutIntentSafe = (
  record: NexusTreasuryPayoutIntentRecord,
): void => {
  assertSafeValue('Nexus treasury payout intent', record)
  assertIdempotencyKeyHash(
    'payout intent idempotency key hash',
    record.idempotencyKeyHash,
  )
  assertSafeRefRequired('payout intent ref', record.payoutIntentRef)
  assertSafeRefRequired('payout target ref', record.payoutTargetRef)
  assertNullableSafeRefRequired(
    'payout target approval ref',
    record.payoutTargetApprovalRef,
  )
  assertSafeRefRequired('policy snapshot ref', record.policySnapshotRef)

  if (safeRefs(record.acceptedWorkRefs).length === 0) {
    throw new NexusTreasuryPayoutLedgerUnsafe({
      reason: 'payout intent requires at least one accepted-work ref.',
    })
  }

  const amount = decodeNexusTreasuryPayoutAmount(record.amount)
  const spendCap = decodeNexusTreasuryPayoutAmount(record.spendCap)

  if (!amountWithinCap(amount, spendCap)) {
    throw new NexusTreasuryPayoutLedgerUnsafe({
      reason: 'payout intent amount must not exceed spend cap.',
    })
  }
}

export const assertNexusTreasuryPayoutAttemptSafe = (
  record: NexusTreasuryPayoutAttemptRecord,
): void => {
  assertSafeValue('Nexus treasury payout attempt', record)
  assertIdempotencyKeyHash(
    'payout attempt idempotency key hash',
    record.idempotencyKeyHash,
  )
  assertSafeRefRequired('payout attempt ref', record.payoutAttemptRef)
  assertSafeRefRequired('payout intent ref', record.payoutIntentRef)
  assertSafeRefRequired('adapter attempt ref', record.adapterAttemptRef)
  assertSafeRefRequired(
    'redacted destination ref',
    record.redactedDestinationRef,
  )
  decodeNexusTreasuryPayoutAmount(record.amount)
}

export const assertNexusTreasuryPayoutLedgerRecordSafe = (
  label: string,
  record: unknown,
): void => {
  assertSafeValue(label, record)
}

type PayoutTargetApprovalRow = Readonly<{
  agent_ref: string | null
  approval_policy_ref: string
  approval_ref: string
  approved_by_ref: string
  archived_at: string | null
  created_at: string
  expires_at: string | null
  id: string
  idempotency_key_hash: string
  owner_user_id: string | null
  public_projection_json: string
  payout_target_ref: string
  pylon_ref: string | null
  redacted_destination_ref: string
  scope_refs_json: string
  status: NexusPayoutTargetApprovalStatus
  updated_at: string
}>

type PayoutIntentRow = Readonly<{
  accepted_work_refs_json: string
  actor_ref: string
  adapter_kind: NexusTreasuryPayoutAdapterKind
  amount_asset: typeof OpenAgentsPaidEndpointAsset.Type
  amount_denomination: typeof OpenAgentsPaidEndpointDenomination.Type
  amount_minor_units: number
  archived_at: string | null
  artanis_dispatch_ref: string | null
  assignment_ref: string | null
  buyer_payment_ref: string | null
  created_at: string
  id: string
  idempotency_key_hash: string
  metadata_refs_json: string
  owner_user_id: string | null
  payout_intent_ref: string
  payout_target_approval_ref: string
  payout_target_ref: string
  policy_snapshot_ref: string
  public_projection_json: string
  pylon_job_ref: string | null
  source_kind: NexusTreasuryPayoutSourceKind
  spend_cap_asset: typeof OpenAgentsPaidEndpointAsset.Type
  spend_cap_denomination: typeof OpenAgentsPaidEndpointDenomination.Type
  spend_cap_amount_minor_units: number
  status: NexusTreasuryPayoutIntentStatus
  updated_at: string
}>

type PayoutAttemptRow = Readonly<{
  adapter_attempt_ref: string
  adapter_kind: NexusTreasuryPayoutAdapterKind
  amount_asset: typeof OpenAgentsPaidEndpointAsset.Type
  amount_denomination: typeof OpenAgentsPaidEndpointDenomination.Type
  amount_minor_units: number
  archived_at: string | null
  created_at: string
  id: string
  idempotency_key_hash: string
  metadata_refs_json: string
  payout_attempt_ref: string
  payout_intent_ref: string
  public_projection_json: string
  redacted_destination_ref: string
  redacted_payment_ref: string | null
  status: NexusTreasuryPayoutAttemptStatus
  updated_at: string
}>

type ReconciliationEventRow = Readonly<{
  adapter_kind: NexusTreasuryPayoutAdapterKind
  archived_at: string | null
  created_at: string
  event_ref: string
  external_event_ref: string
  id: string
  idempotency_key_hash: string
  metadata_refs_json: string
  payout_attempt_ref: string | null
  payout_intent_ref: string | null
  provider_ref: string
  public_projection_json: string
  result_ref: string
  status: NexusTreasuryPayoutReconciliationStatus
}>

type PaymentAuthorityReceiptRow = Readonly<{
  archived_at: string | null
  audience: typeof OpenAgentsPaymentPolicyAudience.Type
  created_at: string
  event_ref: string | null
  id: string
  metadata_refs_json: string
  payout_attempt_ref: string | null
  payout_intent_ref: string
  public_projection_json: string
  receipt_kind: NexusPaymentAuthorityReceiptKind
  receipt_ref: string
}>

const amountFromRow = (
  asset: typeof OpenAgentsPaidEndpointAsset.Type,
  denomination: typeof OpenAgentsPaidEndpointDenomination.Type,
  amountMinorUnits: number,
): NexusTreasuryPayoutAmount => ({
  amountMinorUnits,
  asset,
  denomination,
})

export const nexusPayoutTargetApprovalFromRow = (
  row: PayoutTargetApprovalRow,
): NexusPayoutTargetApprovalRecord => ({
  agentRef: row.agent_ref,
  approvalPolicyRef: row.approval_policy_ref,
  approvalRef: row.approval_ref,
  approvedByRef: row.approved_by_ref,
  archivedAt: row.archived_at,
  createdAt: row.created_at,
  expiresAt: row.expires_at,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  ownerUserId: row.owner_user_id,
  publicProjectionJson: row.public_projection_json,
  payoutTargetRef: row.payout_target_ref,
  pylonRef: row.pylon_ref,
  redactedDestinationRef: row.redacted_destination_ref,
  scopeRefs: parseJsonStringArray(row.scope_refs_json),
  status: row.status,
  updatedAt: row.updated_at,
})

export const nexusTreasuryPayoutIntentFromRow = (
  row: PayoutIntentRow,
): NexusTreasuryPayoutIntentRecord => ({
  acceptedWorkRefs: parseJsonStringArray(row.accepted_work_refs_json),
  actorRef: row.actor_ref,
  adapterKind: row.adapter_kind,
  amount: amountFromRow(
    row.amount_asset,
    row.amount_denomination,
    row.amount_minor_units,
  ),
  archivedAt: row.archived_at,
  artanisDispatchRef: row.artanis_dispatch_ref,
  assignmentRef: row.assignment_ref,
  buyerPaymentRef: row.buyer_payment_ref,
  createdAt: row.created_at,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  metadataRefs: parseJsonStringArray(row.metadata_refs_json),
  ownerUserId: row.owner_user_id,
  payoutIntentRef: row.payout_intent_ref,
  payoutTargetApprovalRef: row.payout_target_approval_ref,
  payoutTargetRef: row.payout_target_ref,
  policySnapshotRef: row.policy_snapshot_ref,
  publicProjectionJson: row.public_projection_json,
  pylonJobRef: row.pylon_job_ref,
  sourceKind: row.source_kind,
  spendCap: amountFromRow(
    row.spend_cap_asset,
    row.spend_cap_denomination,
    row.spend_cap_amount_minor_units,
  ),
  status: row.status,
  updatedAt: row.updated_at,
})

export const nexusTreasuryPayoutAttemptFromRow = (
  row: PayoutAttemptRow,
): NexusTreasuryPayoutAttemptRecord => ({
  adapterAttemptRef: row.adapter_attempt_ref,
  adapterKind: row.adapter_kind,
  amount: amountFromRow(
    row.amount_asset,
    row.amount_denomination,
    row.amount_minor_units,
  ),
  archivedAt: row.archived_at,
  createdAt: row.created_at,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  metadataRefs: parseJsonStringArray(row.metadata_refs_json),
  payoutAttemptRef: row.payout_attempt_ref,
  payoutIntentRef: row.payout_intent_ref,
  publicProjectionJson: row.public_projection_json,
  redactedDestinationRef: row.redacted_destination_ref,
  redactedPaymentRef: row.redacted_payment_ref,
  status: row.status,
  updatedAt: row.updated_at,
})

export const nexusTreasuryPayoutReconciliationEventFromRow = (
  row: ReconciliationEventRow,
): NexusTreasuryPayoutReconciliationEventRecord => ({
  adapterKind: row.adapter_kind,
  archivedAt: row.archived_at,
  createdAt: row.created_at,
  eventRef: row.event_ref,
  externalEventRef: row.external_event_ref,
  id: row.id,
  idempotencyKeyHash: row.idempotency_key_hash,
  metadataRefs: parseJsonStringArray(row.metadata_refs_json),
  payoutAttemptRef: row.payout_attempt_ref,
  payoutIntentRef: row.payout_intent_ref,
  providerRef: row.provider_ref,
  publicProjectionJson: row.public_projection_json,
  resultRef: row.result_ref,
  status: row.status,
})

export const nexusPaymentAuthorityReceiptFromRow = (
  row: PaymentAuthorityReceiptRow,
): NexusPaymentAuthorityReceiptRecord => ({
  archivedAt: row.archived_at,
  audience: row.audience,
  createdAt: row.created_at,
  eventRef: row.event_ref,
  id: row.id,
  metadataRefs: parseJsonStringArray(row.metadata_refs_json),
  payoutAttemptRef: row.payout_attempt_ref,
  payoutIntentRef: row.payout_intent_ref,
  publicProjectionJson: row.public_projection_json,
  receiptKind: row.receipt_kind,
  receiptRef: row.receipt_ref,
})

export const projectNexusTreasuryPayoutLedgerRecord = (
  recordKind: NexusTreasuryPayoutLedgerRecordKind,
  record: NexusTreasuryPayoutLedgerRecord,
  audience: typeof OpenAgentsPaymentPolicyAudience.Type,
): NexusTreasuryPayoutLedgerProjection => {
  assertNexusTreasuryPayoutLedgerRecordSafe(
    'Nexus treasury payout projection input',
    record,
  )

  const operator = audience === 'operator'
  const customerOrAgent = audience === 'customer' || audience === 'agent'
  const metadataRefs =
    'metadataRefs' in record ? safeRefs(record.metadataRefs) : []
  const payoutIntentRef =
    'payoutIntentRef' in record ? nullableSafeRef(record.payoutIntentRef) : null
  const payoutAttemptRef =
    'payoutAttemptRef' in record
      ? nullableSafeRef(record.payoutAttemptRef)
      : null
  const payoutTargetRef =
    'payoutTargetRef' in record ? nullableSafeRef(record.payoutTargetRef) : null
  const payoutTargetApprovalRef =
    'payoutTargetApprovalRef' in record
      ? nullableSafeRef(record.payoutTargetApprovalRef)
      : 'approvalRef' in record
        ? nullableSafeRef(record.approvalRef)
        : null
  const amount = 'amount' in record ? record.amount : null
  const adapterKind = 'adapterKind' in record ? record.adapterKind : null
  const assignmentRef =
    'assignmentRef' in record ? nullableSafeRef(record.assignmentRef) : null
  const ownerUserId =
    'ownerUserId' in record ? nullableSafeRef(record.ownerUserId) : null
  const publicProjectionJson =
    'publicProjectionJson' in record
      ? safeJsonString(record.publicProjectionJson)
      : '{}'
  const receiptRef =
    'receiptRef' in record ? nullableSafeRef(record.receiptRef) : null
  const redactedDestinationRef =
    'redactedDestinationRef' in record && (operator || customerOrAgent)
      ? nullableSafeRef(record.redactedDestinationRef)
      : null
  const redactedPaymentRef =
    'redactedPaymentRef' in record && (operator || customerOrAgent)
      ? nullableSafeRef(record.redactedPaymentRef)
      : null

  return {
    adapterKind,
    amount,
    assignmentRef,
    audience,
    metadataRefs: operator ? metadataRefs : [],
    operatorRefs: operator
      ? safeRefs([
          ...metadataRefs,
          ...('actorRef' in record ? [record.actorRef] : []),
          ...('adapterAttemptRef' in record ? [record.adapterAttemptRef] : []),
          ...('externalEventRef' in record ? [record.externalEventRef] : []),
          ...('providerRef' in record ? [record.providerRef] : []),
        ])
      : [],
    ownerUserId: operator ? ownerUserId : null,
    payoutAttemptRef,
    payoutIntentRef,
    payoutTargetApprovalRef,
    payoutTargetRef: operator ? payoutTargetRef : null,
    publicProjectionJson,
    receiptRef,
    recordKind,
    redactedDestinationRef,
    redactedPaymentRef,
    status: 'status' in record ? record.status : record.receiptKind,
  }
}

export const nexusTreasuryPayoutLedgerProjectionHasPrivateMaterial = (
  projection: NexusTreasuryPayoutLedgerProjection,
): boolean => scanForUnsafeMaterial(projection) !== undefined

const storageError = (
  operation: string,
  error: unknown,
): NexusTreasuryPayoutLedgerStorageError =>
  new NexusTreasuryPayoutLedgerStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

export type NexusTreasuryPayoutLedgerStore = Readonly<{
  createPayoutAttempt: (
    record: NexusTreasuryPayoutAttemptRecord,
  ) => Promise<void>
  createPayoutIntent: (record: NexusTreasuryPayoutIntentRecord) => Promise<void>
  createPayoutTargetApproval: (
    record: NexusPayoutTargetApprovalRecord,
  ) => Promise<void>
  createPaymentAuthorityReceipt: (
    record: NexusPaymentAuthorityReceiptRecord,
  ) => Promise<void>
  createReconciliationEvent: (
    record: NexusTreasuryPayoutReconciliationEventRecord,
  ) => Promise<void>
  createReleaseGate: (record: NexusReleaseGateRecord) => Promise<void>
  listPaymentAuthorityReceipts: (
    limit: number,
  ) => Promise<ReadonlyArray<NexusPaymentAuthorityReceiptRecord>>
  readPayoutAttemptByRef: (
    payoutAttemptRef: string,
  ) => Promise<NexusTreasuryPayoutAttemptRecord | undefined>
  readPayoutAttemptByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<NexusTreasuryPayoutAttemptRecord | undefined>
  readPayoutIntentByIdempotencyKeyHash: (
    idempotencyKeyHash: string,
  ) => Promise<NexusTreasuryPayoutIntentRecord | undefined>
  readPayoutIntentByBuyerPaymentRef: (
    buyerPaymentRef: string,
  ) => Promise<NexusTreasuryPayoutIntentRecord | undefined>
  readPayoutIntentByRef: (
    payoutIntentRef: string,
  ) => Promise<NexusTreasuryPayoutIntentRecord | undefined>
  readPaymentAuthorityReceiptByRef: (
    receiptRef: string,
  ) => Promise<NexusPaymentAuthorityReceiptRecord | undefined>
  readReconciliationEventByRef: (
    eventRef: string,
  ) => Promise<NexusTreasuryPayoutReconciliationEventRecord | undefined>
}>

/**
 * KS-8.8 (#8319): D1 stays the SOLE payout authority — every read and
 * every write below runs against D1 exactly as before this lane. On a
 * `TreasuryDatabase` seam handle, each append-only create additionally
 * read-back-mirrors the resolved row into Postgres fail-soft (keys only in
 * diagnostics). The dispatcher's decision reads have NO Postgres twin, so
 * no flag can ever make the mirror drive (or double-drive) a dispatch.
 */
export const makeD1NexusTreasuryPayoutLedgerStore = (
  database: TreasuryDatabase,
): NexusTreasuryPayoutLedgerStore => {
  const db = treasuryAuthorityDb(database)
  const readPayoutIntentByRef = async (
    payoutIntentRef: string,
  ): Promise<NexusTreasuryPayoutIntentRecord | undefined> => {
    try {
      const row = await db
        .prepare(
          `SELECT *
             FROM nexus_treasury_payout_intents
            WHERE payout_intent_ref = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(payoutIntentRef)
        .first<PayoutIntentRow>()

      return row === null ? undefined : nexusTreasuryPayoutIntentFromRow(row)
    } catch (error) {
      throw storageError(
        'nexusTreasuryPayoutLedger.readPayoutIntentByRef',
        error,
      )
    }
  }
  const readPayoutAttemptByIdempotencyKeyHash = async (
    idempotencyKeyHash: string,
  ): Promise<NexusTreasuryPayoutAttemptRecord | undefined> => {
    try {
      const row = await db
        .prepare(
          `SELECT *
             FROM nexus_treasury_payout_attempts
            WHERE idempotency_key_hash = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(idempotencyKeyHash)
        .first<PayoutAttemptRow>()

      return row === null ? undefined : nexusTreasuryPayoutAttemptFromRow(row)
    } catch (error) {
      throw storageError(
        'nexusTreasuryPayoutLedger.readPayoutAttemptByIdempotencyKeyHash',
        error,
      )
    }
  }

  return {
    createPayoutAttempt: async record => {
      assertNexusTreasuryPayoutAttemptSafe(record)

      const intent = await readPayoutIntentByRef(record.payoutIntentRef)

      if (intent === undefined) {
        throw new NexusTreasuryPayoutLedgerUnsafe({
          reason: 'payout attempt requires an existing payout intent.',
        })
      }

      try {
        await db
          .prepare(
            `INSERT OR IGNORE INTO nexus_treasury_payout_attempts
             (id, payout_attempt_ref, payout_intent_ref, idempotency_key_hash,
              adapter_kind, adapter_attempt_ref, status, redacted_payment_ref,
              redacted_destination_ref, amount_asset, amount_denomination,
              amount_minor_units, metadata_refs_json, public_projection_json,
              created_at, updated_at, archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            record.id,
            record.payoutAttemptRef,
            record.payoutIntentRef,
            record.idempotencyKeyHash,
            record.adapterKind,
            record.adapterAttemptRef,
            record.status,
            record.redactedPaymentRef,
            record.redactedDestinationRef,
            record.amount.asset,
            record.amount.denomination,
            record.amount.amountMinorUnits,
            jsonStringArray(record.metadataRefs),
            record.publicProjectionJson,
            record.createdAt,
            record.updatedAt,
            record.archivedAt,
          )
          .run()
      } catch (error) {
        throw storageError(
          'nexusTreasuryPayoutLedger.createPayoutAttempt',
          error,
        )
      }
      await mirrorTreasuryRows(
        database,
        'nexus_treasury_payout_attempts',
        'payout_attempt_ref',
        [record.payoutAttemptRef],
      )
    },
    createPayoutIntent: async record => {
      assertNexusTreasuryPayoutIntentSafe(record)

      try {
        await db
          .prepare(
            `INSERT OR IGNORE INTO nexus_treasury_payout_intents
             (id, payout_intent_ref, idempotency_key_hash, actor_ref,
              owner_user_id, source_kind, buyer_payment_ref,
              accepted_work_refs_json, assignment_ref, artanis_dispatch_ref,
              pylon_job_ref, payout_target_ref, payout_target_approval_ref,
              adapter_kind, amount_asset, amount_denomination,
              amount_minor_units, spend_cap_asset, spend_cap_denomination,
              spend_cap_amount_minor_units, policy_snapshot_ref, status,
              metadata_refs_json, public_projection_json, created_at,
              updated_at, archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            record.id,
            record.payoutIntentRef,
            record.idempotencyKeyHash,
            record.actorRef,
            record.ownerUserId,
            record.sourceKind,
            record.buyerPaymentRef,
            jsonStringArray(record.acceptedWorkRefs),
            record.assignmentRef,
            record.artanisDispatchRef,
            record.pylonJobRef,
            record.payoutTargetRef,
            record.payoutTargetApprovalRef,
            record.adapterKind,
            record.amount.asset,
            record.amount.denomination,
            record.amount.amountMinorUnits,
            record.spendCap.asset,
            record.spendCap.denomination,
            record.spendCap.amountMinorUnits,
            record.policySnapshotRef,
            record.status,
            jsonStringArray(record.metadataRefs),
            record.publicProjectionJson,
            record.createdAt,
            record.updatedAt,
            record.archivedAt,
          )
          .run()
      } catch (error) {
        throw storageError(
          'nexusTreasuryPayoutLedger.createPayoutIntent',
          error,
        )
      }

      // `INSERT OR IGNORE` silently drops the row on any constraint conflict
      // (a UNIQUE collision on a different `id` / `payout_intent_ref` /
      // `idempotency_key_hash`, or a foreign-key violation) and still resolves
      // successfully. That turned a real persistence failure into a misleading
      // downstream `payout_intent_not_found` at dispatch (openagents #5232) —
      // moving no money and writing no receipt. Confirm the intent is durably
      // present by ref before returning. A genuine idempotent replay re-inserts
      // the identical row, finds it here, and succeeds; a true silent drop now
      // fails loudly at the persistence boundary where it belongs, exactly as
      // createPayoutAttempt already guards its parent intent.
      const persisted = await readPayoutIntentByRef(record.payoutIntentRef)

      if (persisted === undefined) {
        throw new NexusTreasuryPayoutLedgerUnsafe({
          reason:
            'payout intent insert was silently ignored (constraint conflict); the intent is not durably persisted.',
        })
      }

      await mirrorTreasuryRows(
        database,
        'nexus_treasury_payout_intents',
        'payout_intent_ref',
        [record.payoutIntentRef],
      )
    },
    createPayoutTargetApproval: async record => {
      assertNexusTreasuryPayoutLedgerRecordSafe(
        'Nexus payout target approval',
        record,
      )
      assertIdempotencyKeyHash(
        'payout target approval idempotency key hash',
        record.idempotencyKeyHash,
      )
      assertSafeRefRequired('payout target approval ref', record.approvalRef)
      assertSafeRefRequired('payout target ref', record.payoutTargetRef)
      assertSafeRefRequired(
        'redacted destination ref',
        record.redactedDestinationRef,
      )

      try {
        await db
          .prepare(
            `INSERT OR IGNORE INTO nexus_payout_target_approvals
             (id, approval_ref, idempotency_key_hash, payout_target_ref,
              redacted_destination_ref, owner_user_id, agent_ref, pylon_ref,
              status, approved_by_ref, approval_policy_ref, scope_refs_json,
              public_projection_json, created_at, updated_at, expires_at,
              archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            record.id,
            record.approvalRef,
            record.idempotencyKeyHash,
            record.payoutTargetRef,
            record.redactedDestinationRef,
            record.ownerUserId,
            record.agentRef,
            record.pylonRef,
            record.status,
            record.approvedByRef,
            record.approvalPolicyRef,
            jsonStringArray(record.scopeRefs),
            record.publicProjectionJson,
            record.createdAt,
            record.updatedAt,
            record.expiresAt,
            record.archivedAt,
          )
          .run()
      } catch (error) {
        throw storageError(
          'nexusTreasuryPayoutLedger.createPayoutTargetApproval',
          error,
        )
      }
      await mirrorTreasuryRows(
        database,
        'nexus_payout_target_approvals',
        'approval_ref',
        [record.approvalRef],
      )
    },
    createPaymentAuthorityReceipt: async record => {
      assertNexusTreasuryPayoutLedgerRecordSafe(
        'Nexus payment authority receipt',
        record,
      )

      try {
        await db
          .prepare(
            `INSERT OR IGNORE INTO nexus_payment_authority_receipts
             (id, receipt_ref, payout_intent_ref, payout_attempt_ref,
              event_ref, receipt_kind, audience, metadata_refs_json,
              public_projection_json, created_at, archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            record.id,
            record.receiptRef,
            record.payoutIntentRef,
            record.payoutAttemptRef,
            record.eventRef,
            record.receiptKind,
            record.audience,
            jsonStringArray(record.metadataRefs),
            record.publicProjectionJson,
            record.createdAt,
            record.archivedAt,
          )
          .run()
      } catch (error) {
        throw storageError(
          'nexusTreasuryPayoutLedger.createPaymentAuthorityReceipt',
          error,
        )
      }
      await mirrorTreasuryRows(
        database,
        'nexus_payment_authority_receipts',
        'receipt_ref',
        [record.receiptRef],
      )
    },
    createReconciliationEvent: async record => {
      assertNexusTreasuryPayoutLedgerRecordSafe(
        'Nexus treasury payout reconciliation event',
        record,
      )
      assertIdempotencyKeyHash(
        'payout reconciliation idempotency key hash',
        record.idempotencyKeyHash,
      )

      try {
        await db
          .prepare(
            `INSERT OR IGNORE INTO nexus_treasury_payout_reconciliation_events
             (id, event_ref, idempotency_key_hash, provider_ref,
              external_event_ref, adapter_kind, payout_intent_ref,
              payout_attempt_ref, status, result_ref, metadata_refs_json,
              public_projection_json, created_at, archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            record.id,
            record.eventRef,
            record.idempotencyKeyHash,
            record.providerRef,
            record.externalEventRef,
            record.adapterKind,
            record.payoutIntentRef,
            record.payoutAttemptRef,
            record.status,
            record.resultRef,
            jsonStringArray(record.metadataRefs),
            record.publicProjectionJson,
            record.createdAt,
            record.archivedAt,
          )
          .run()
      } catch (error) {
        throw storageError(
          'nexusTreasuryPayoutLedger.createReconciliationEvent',
          error,
        )
      }
      await mirrorTreasuryRows(
        database,
        'nexus_treasury_payout_reconciliation_events',
        'event_ref',
        [record.eventRef],
      )
    },
    createReleaseGate: async record => {
      assertNexusTreasuryPayoutLedgerRecordSafe('Nexus release gate', record)
      assertIdempotencyKeyHash(
        'release gate idempotency key hash',
        record.idempotencyKeyHash,
      )

      try {
        await db
          .prepare(
            `INSERT OR IGNORE INTO nexus_release_gates
             (id, gate_ref, idempotency_key_hash, gate_kind, status,
              evidence_refs_json, blocker_refs_json, public_projection_json,
              created_at, updated_at, archived_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            record.id,
            record.gateRef,
            record.idempotencyKeyHash,
            record.gateKind,
            record.status,
            jsonStringArray(record.evidenceRefs),
            jsonStringArray(record.blockerRefs),
            record.publicProjectionJson,
            record.createdAt,
            record.updatedAt,
            record.archivedAt,
          )
          .run()
      } catch (error) {
        throw storageError('nexusTreasuryPayoutLedger.createReleaseGate', error)
      }
      await mirrorTreasuryRows(database, 'nexus_release_gates', 'gate_ref', [
        record.gateRef,
      ])
    },
    readPayoutAttemptByRef: async payoutAttemptRef => {
      try {
        const row = await db
          .prepare(
            `SELECT *
               FROM nexus_treasury_payout_attempts
              WHERE payout_attempt_ref = ?
                AND archived_at IS NULL
              LIMIT 1`,
          )
          .bind(payoutAttemptRef)
          .first<PayoutAttemptRow>()

        return row === null ? undefined : nexusTreasuryPayoutAttemptFromRow(row)
      } catch (error) {
        throw storageError(
          'nexusTreasuryPayoutLedger.readPayoutAttemptByRef',
          error,
        )
      }
    },
    readPayoutAttemptByIdempotencyKeyHash,
    listPaymentAuthorityReceipts: async limit => {
      try {
        const result = await db
          .prepare(
            `SELECT *
               FROM nexus_payment_authority_receipts
              WHERE archived_at IS NULL
              ORDER BY created_at DESC
              LIMIT ?`,
          )
          .bind(limit)
          .all<PaymentAuthorityReceiptRow>()

        return (result.results ?? []).map(nexusPaymentAuthorityReceiptFromRow)
      } catch (error) {
        throw storageError(
          'nexusTreasuryPayoutLedger.listPaymentAuthorityReceipts',
          error,
        )
      }
    },
    readPaymentAuthorityReceiptByRef: async receiptRef => {
      try {
        const row = await db
          .prepare(
            `SELECT *
               FROM nexus_payment_authority_receipts
              WHERE receipt_ref = ?
                AND archived_at IS NULL
              LIMIT 1`,
          )
          .bind(receiptRef)
          .first<PaymentAuthorityReceiptRow>()

        return row === null
          ? undefined
          : nexusPaymentAuthorityReceiptFromRow(row)
      } catch (error) {
        throw storageError(
          'nexusTreasuryPayoutLedger.readPaymentAuthorityReceiptByRef',
          error,
        )
      }
    },
    readPayoutIntentByIdempotencyKeyHash: async idempotencyKeyHash => {
      try {
        const row = await db
          .prepare(
            `SELECT *
               FROM nexus_treasury_payout_intents
              WHERE idempotency_key_hash = ?
                AND archived_at IS NULL
              LIMIT 1`,
          )
          .bind(idempotencyKeyHash)
          .first<PayoutIntentRow>()

        return row === null ? undefined : nexusTreasuryPayoutIntentFromRow(row)
      } catch (error) {
        throw storageError(
          'nexusTreasuryPayoutLedger.readPayoutIntentByIdempotencyKeyHash',
          error,
        )
      }
    },
    readPayoutIntentByBuyerPaymentRef: async buyerPaymentRef => {
      try {
        const row = await db
          .prepare(
            `SELECT *
               FROM nexus_treasury_payout_intents
              WHERE buyer_payment_ref = ?
                AND archived_at IS NULL
              LIMIT 1`,
          )
          .bind(buyerPaymentRef)
          .first<PayoutIntentRow>()

        return row === null ? undefined : nexusTreasuryPayoutIntentFromRow(row)
      } catch (error) {
        throw storageError(
          'nexusTreasuryPayoutLedger.readPayoutIntentByBuyerPaymentRef',
          error,
        )
      }
    },
    readPayoutIntentByRef,
    readReconciliationEventByRef: async eventRef => {
      try {
        const row = await db
          .prepare(
            `SELECT *
               FROM nexus_treasury_payout_reconciliation_events
              WHERE event_ref = ?
                AND archived_at IS NULL
              LIMIT 1`,
          )
          .bind(eventRef)
          .first<ReconciliationEventRow>()

        return row === null
          ? undefined
          : nexusTreasuryPayoutReconciliationEventFromRow(row)
      } catch (error) {
        throw storageError(
          'nexusTreasuryPayoutLedger.readReconciliationEventByRef',
          error,
        )
      }
    },
  }
}
