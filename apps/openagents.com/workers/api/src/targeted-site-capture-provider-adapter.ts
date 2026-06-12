import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  isTargetedSiteCaptureFetchable,
  type TargetedSiteCapturePolicyEventRecord,
} from './targeted-site-capture-policy'

export const TargetedSiteCaptureProviderKind = S.Literals([
  'first_party_worker',
  'browser_run',
  'firecrawl',
  'browserless',
  'browserbase',
  'apify',
  'container',
])
export type TargetedSiteCaptureProviderKind =
  typeof TargetedSiteCaptureProviderKind.Type

export const TargetedSiteCaptureProviderState = S.Literals([
  'requested',
  'approved_fallback',
  'benchmark',
  'denied',
  'failed',
  'partial',
  'succeeded',
  'manual_review',
  'archived',
])
export type TargetedSiteCaptureProviderState =
  typeof TargetedSiteCaptureProviderState.Type

export const TargetedSiteCaptureProviderReason = S.Literals([
  'first_party_default',
  'static_insufficient',
  'rendered_insufficient',
  'paid_escalation_approved',
  'benchmark_quality_check',
  'cost_not_approved',
  'provider_unavailable',
  'provider_error',
  'manual_review',
  'policy_not_fetchable',
  'bot_protection_or_login',
])
export type TargetedSiteCaptureProviderReason =
  typeof TargetedSiteCaptureProviderReason.Type

export const TargetedSiteCaptureProviderAdapterRunRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  campaignId: S.String,
  capturePolicyEventId: S.String,
  completedAt: S.NullOr(S.String),
  costRef: S.NullOr(S.String),
  createdAt: S.String,
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  normalizedDomain: S.String,
  outputPackRef: S.NullOr(S.String),
  paidEscalationRef: S.NullOr(S.String),
  prospectId: S.NullOr(S.String),
  providerKind: TargetedSiteCaptureProviderKind,
  providerReceiptRef: S.NullOr(S.String),
  providerRequestRef: S.NullOr(S.String),
  reason: TargetedSiteCaptureProviderReason,
  renderedCaptureRunId: S.NullOr(S.String),
  requestedAt: S.String,
  state: TargetedSiteCaptureProviderState,
  staticCaptureRunId: S.NullOr(S.String),
  usageRef: S.NullOr(S.String),
})
export type TargetedSiteCaptureProviderAdapterRunRecord =
  typeof TargetedSiteCaptureProviderAdapterRunRecord.Type

export const PublicTargetedSiteCaptureProviderAdapterProjection = S.Struct({
  campaignId: S.String,
  completedAt: S.NullOr(S.String),
  normalizedDomain: S.String,
  outputAvailable: S.Boolean,
  prospectId: S.NullOr(S.String),
  providerKind: TargetedSiteCaptureProviderKind,
  requestedAt: S.String,
  state: TargetedSiteCaptureProviderState,
})
export type PublicTargetedSiteCaptureProviderAdapterProjection =
  typeof PublicTargetedSiteCaptureProviderAdapterProjection.Type

export const OperatorTargetedSiteCaptureProviderAdapterProjection = S.Struct({
  campaignId: S.String,
  capturePolicyEventId: S.String,
  completedAt: S.NullOr(S.String),
  costRef: S.NullOr(S.String),
  hasMetadata: S.Boolean,
  normalizedDomain: S.String,
  outputPackRef: S.NullOr(S.String),
  paidEscalationRef: S.NullOr(S.String),
  prospectId: S.NullOr(S.String),
  providerKind: TargetedSiteCaptureProviderKind,
  providerReceiptRef: S.NullOr(S.String),
  providerRequestRef: S.NullOr(S.String),
  reason: TargetedSiteCaptureProviderReason,
  renderedCaptureRunId: S.NullOr(S.String),
  requestedAt: S.String,
  state: TargetedSiteCaptureProviderState,
  staticCaptureRunId: S.NullOr(S.String),
  usageRef: S.NullOr(S.String),
})
export type OperatorTargetedSiteCaptureProviderAdapterProjection =
  typeof OperatorTargetedSiteCaptureProviderAdapterProjection.Type

export type RecordTargetedSiteCaptureProviderAdapterRunInput = Readonly<{
  capturePolicyEvent: TargetedSiteCapturePolicyEventRecord
  completedAt?: string | undefined
  costRef?: string | undefined
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  outputPackRef?: string | undefined
  paidEscalationRef?: string | undefined
  providerKind: TargetedSiteCaptureProviderKind
  providerReceiptRef?: string | undefined
  providerRequestRef?: string | undefined
  reason?: TargetedSiteCaptureProviderReason | undefined
  renderedCaptureRunId?: string | undefined
  requestedAt?: string | undefined
  state?: TargetedSiteCaptureProviderState | undefined
  staticCaptureRunId?: string | undefined
  usageRef?: string | undefined
}>

type TargetedSiteCaptureProviderAdapterRunRow = Readonly<{
  archived_at: string | null
  campaign_id: string
  capture_policy_event_id: string
  completed_at: string | null
  cost_ref: string | null
  created_at: string
  id: string
  idempotency_key: string
  metadata_json: string
  normalized_domain: string
  output_pack_ref: string | null
  paid_escalation_ref: string | null
  prospect_id: string | null
  provider_kind: TargetedSiteCaptureProviderKind
  provider_receipt_ref: string | null
  provider_request_ref: string | null
  reason: TargetedSiteCaptureProviderReason
  rendered_capture_run_id: string | null
  requested_at: string
  state: TargetedSiteCaptureProviderState
  static_capture_run_id: string | null
  usage_ref: string | null
}>

type TargetedSiteCaptureProviderAdapterRunFilter =
  | Readonly<{ key: 'campaign_id'; value: string }>
  | Readonly<{ key: 'prospect_id'; value: string }>
  | Readonly<{ key: 'normalized_domain'; value: string }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_DOMAIN_PATTERN =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{1,62}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?payload|browser[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic|bypass|captcha|cloudflare challenge|headless stealth|anti-bot)|@/i

const paidProviderKinds: ReadonlySet<TargetedSiteCaptureProviderKind> = new Set([
  'firecrawl',
  'browserless',
  'browserbase',
  'apify',
  'container',
])

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const isSafeRef = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value) && textIsSafe(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!isSafeRef(value)) {
    throw new TargetedSiteCaptureProviderAdapterValidationError({
      reason: `${field} must be a public-safe ref without raw provider payload, browser-log, contact, payment, wallet, or bypass material.`,
    })
  }
}

const assertSafeDomain = (value: string): void => {
  if (!SAFE_DOMAIN_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteCaptureProviderAdapterValidationError({
      reason: 'normalizedDomain must be a public-safe normalized domain.',
    })
  }
}

const assertSafeMetadata = (
  metadata: Readonly<Record<string, unknown>> | undefined,
): void => {
  if (metadata === undefined) {
    return
  }

  const json = JSON.stringify(metadata)

  if (containsProviderSecretMaterial(json) || PROHIBITED_TEXT_PATTERN.test(json)) {
    throw new TargetedSiteCaptureProviderAdapterValidationError({
      reason:
        'metadata must not contain raw provider payload, browser-log, contact, payment, wallet, or bypass material.',
    })
  }
}

const assertFetchablePolicy = (
  policy: TargetedSiteCapturePolicyEventRecord,
): void => {
  assertSafeDomain(policy.normalizedDomain)

  if (!isTargetedSiteCaptureFetchable(policy)) {
    throw new TargetedSiteCaptureProviderAdapterValidationError({
      reason:
        'provider adapter runs require an explicit allowed or paid-escalation capture policy event.',
    })
  }
}

const requiresPaidEscalation = (
  input: RecordTargetedSiteCaptureProviderAdapterRunInput,
): boolean =>
  paidProviderKinds.has(input.providerKind) &&
  (input.state === 'approved_fallback' ||
    input.state === 'succeeded' ||
    input.state === 'partial' ||
    input.reason === 'paid_escalation_approved')

const assertPaidEscalation = (
  input: RecordTargetedSiteCaptureProviderAdapterRunInput,
): void => {
  if (!requiresPaidEscalation(input)) {
    return
  }

  const hasPolicyPaidEscalation =
    input.capturePolicyEvent.decision === 'paid_escalation' &&
    input.capturePolicyEvent.paidEscalationRef !== null
  const hasInputPaidEscalation = input.paidEscalationRef !== undefined

  if (!hasPolicyPaidEscalation && !hasInputPaidEscalation) {
    throw new TargetedSiteCaptureProviderAdapterValidationError({
      reason:
        'paid provider fallback requires explicit paid-escalation policy evidence.',
    })
  }
}

const assertValidInput = (
  input: RecordTargetedSiteCaptureProviderAdapterRunInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('staticCaptureRunId', input.staticCaptureRunId)
  assertSafeRef('renderedCaptureRunId', input.renderedCaptureRunId)
  assertSafeRef('paidEscalationRef', input.paidEscalationRef)
  assertSafeRef('providerRequestRef', input.providerRequestRef)
  assertSafeRef('providerReceiptRef', input.providerReceiptRef)
  assertSafeRef('outputPackRef', input.outputPackRef)
  assertSafeRef('usageRef', input.usageRef)
  assertSafeRef('costRef', input.costRef)
  assertSafeMetadata(input.metadata)
  assertFetchablePolicy(input.capturePolicyEvent)
  assertPaidEscalation(input)
}

const stateAndReason = (
  input: RecordTargetedSiteCaptureProviderAdapterRunInput,
): readonly [TargetedSiteCaptureProviderState, TargetedSiteCaptureProviderReason] => {
  if (input.state !== undefined && input.reason !== undefined) {
    return [input.state, input.reason]
  }

  if (input.providerKind === 'first_party_worker') {
    return [input.state ?? 'requested', input.reason ?? 'first_party_default']
  }

  if (input.providerKind === 'browser_run') {
    return [input.state ?? 'benchmark', input.reason ?? 'benchmark_quality_check']
  }

  return [input.state ?? 'requested', input.reason ?? 'static_insufficient']
}

const metadataFromJson = (json: string): Readonly<Record<string, unknown>> =>
  parseJsonRecord(json) ?? {}

export class TargetedSiteCaptureProviderAdapterValidationError extends S.TaggedErrorClass<TargetedSiteCaptureProviderAdapterValidationError>()(
  'TargetedSiteCaptureProviderAdapterValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteCaptureProviderAdapterStorageError extends S.TaggedErrorClass<TargetedSiteCaptureProviderAdapterStorageError>()(
  'TargetedSiteCaptureProviderAdapterStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

const runFromRow = (
  row: TargetedSiteCaptureProviderAdapterRunRow,
): TargetedSiteCaptureProviderAdapterRunRecord => ({
  archivedAt: row.archived_at,
  campaignId: row.campaign_id,
  capturePolicyEventId: row.capture_policy_event_id,
  completedAt: row.completed_at,
  costRef: row.cost_ref,
  createdAt: row.created_at,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: metadataFromJson(row.metadata_json),
  normalizedDomain: row.normalized_domain,
  outputPackRef: row.output_pack_ref,
  paidEscalationRef: row.paid_escalation_ref,
  prospectId: row.prospect_id,
  providerKind: row.provider_kind,
  providerReceiptRef: row.provider_receipt_ref,
  providerRequestRef: row.provider_request_ref,
  reason: row.reason,
  renderedCaptureRunId: row.rendered_capture_run_id,
  requestedAt: row.requested_at,
  state: row.state,
  staticCaptureRunId: row.static_capture_run_id,
  usageRef: row.usage_ref,
})

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<TargetedSiteCaptureProviderAdapterRunRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM targeted_site_capture_provider_adapter_runs
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<TargetedSiteCaptureProviderAdapterRunRow>()

  return row === null ? null : runFromRow(row)
}

const listByFilter = async (
  db: D1Database,
  filter: TargetedSiteCaptureProviderAdapterRunFilter,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteCaptureProviderAdapterRunRecord>> => {
  if (filter.key === 'normalized_domain') {
    assertSafeDomain(filter.value)
  } else {
    assertSafeRef(filter.key, filter.value)
  }

  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM targeted_site_capture_provider_adapter_runs
        WHERE ${filter.key} = ?
          AND archived_at IS NULL
        ORDER BY requested_at DESC
        LIMIT ?`,
    )
    .bind(filter.value, safeLimit)
    .all<TargetedSiteCaptureProviderAdapterRunRow>()

  return (rows.results ?? []).map(runFromRow)
}

export const publicTargetedSiteCaptureProviderAdapterProjection = (
  record: TargetedSiteCaptureProviderAdapterRunRecord,
): PublicTargetedSiteCaptureProviderAdapterProjection => ({
  campaignId: record.campaignId,
  completedAt: record.completedAt,
  normalizedDomain: record.normalizedDomain,
  outputAvailable: record.outputPackRef !== null,
  prospectId: record.prospectId,
  providerKind: record.providerKind,
  requestedAt: record.requestedAt,
  state: record.state,
})

export const operatorTargetedSiteCaptureProviderAdapterProjection = (
  record: TargetedSiteCaptureProviderAdapterRunRecord,
): OperatorTargetedSiteCaptureProviderAdapterProjection => ({
  campaignId: record.campaignId,
  capturePolicyEventId: record.capturePolicyEventId,
  completedAt: record.completedAt,
  costRef: record.costRef,
  hasMetadata: Object.keys(record.metadata).length > 0,
  normalizedDomain: record.normalizedDomain,
  outputPackRef: record.outputPackRef,
  paidEscalationRef: record.paidEscalationRef,
  prospectId: record.prospectId,
  providerKind: record.providerKind,
  providerReceiptRef: record.providerReceiptRef,
  providerRequestRef: record.providerRequestRef,
  reason: record.reason,
  renderedCaptureRunId: record.renderedCaptureRunId,
  requestedAt: record.requestedAt,
  state: record.state,
  staticCaptureRunId: record.staticCaptureRunId,
  usageRef: record.usageRef,
})

export const recordTargetedSiteCaptureProviderAdapterRun = async (
  db: D1Database,
  input: RecordTargetedSiteCaptureProviderAdapterRunInput,
): Promise<TargetedSiteCaptureProviderAdapterRunRecord> => {
  assertValidInput(input)

  const existing = await readByIdempotencyKey(db, input.idempotencyKey)

  if (existing !== null) {
    return existing
  }

  const nowIso = currentIsoTimestamp()
  const policy = input.capturePolicyEvent
  const [state, reason] = stateAndReason(input)
  const id = input.id ?? compactRandomId('targeted_site_capture_provider')

  await db
    .prepare(
      `INSERT OR IGNORE INTO targeted_site_capture_provider_adapter_runs (
         id,
         idempotency_key,
         campaign_id,
         prospect_id,
         normalized_domain,
         capture_policy_event_id,
         static_capture_run_id,
         rendered_capture_run_id,
         provider_kind,
         state,
         reason,
         paid_escalation_ref,
         provider_request_ref,
         provider_receipt_ref,
         output_pack_ref,
         usage_ref,
         cost_ref,
         metadata_json,
         requested_at,
         completed_at,
         created_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .bind(
      id,
      input.idempotencyKey,
      policy.campaignId,
      policy.prospectId,
      policy.normalizedDomain,
      policy.id,
      input.staticCaptureRunId ?? null,
      input.renderedCaptureRunId ?? null,
      input.providerKind,
      state,
      reason,
      input.paidEscalationRef ?? policy.paidEscalationRef,
      input.providerRequestRef ?? null,
      input.providerReceiptRef ?? null,
      input.outputPackRef ?? null,
      input.usageRef ?? null,
      input.costRef ?? null,
      JSON.stringify(input.metadata ?? {}),
      input.requestedAt ?? nowIso,
      input.completedAt ?? null,
      nowIso,
    )
    .run()

  const record = await readByIdempotencyKey(db, input.idempotencyKey)

  if (record === null) {
    throw new TargetedSiteCaptureProviderAdapterStorageError({
      operation: 'recordTargetedSiteCaptureProviderAdapterRun.readByIdempotencyKey',
      reason: 'inserted or existing targeted Site capture provider adapter run was not readable.',
    })
  }

  return record
}

export const listTargetedSiteCaptureProviderAdapterRunsByCampaign = async (
  db: D1Database,
  campaignId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteCaptureProviderAdapterRunRecord>> =>
  listByFilter(db, { key: 'campaign_id', value: campaignId }, limit)

export const listTargetedSiteCaptureProviderAdapterRunsByProspect = async (
  db: D1Database,
  prospectId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteCaptureProviderAdapterRunRecord>> =>
  listByFilter(db, { key: 'prospect_id', value: prospectId }, limit)

export const listTargetedSiteCaptureProviderAdapterRunsByDomain = async (
  db: D1Database,
  normalizedDomain: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteCaptureProviderAdapterRunRecord>> =>
  listByFilter(db, { key: 'normalized_domain', value: normalizedDomain }, limit)
