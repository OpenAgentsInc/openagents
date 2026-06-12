import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  isTargetedSiteCaptureFetchable,
  type TargetedSiteCapturePolicyEventRecord,
} from './targeted-site-capture-policy'

export const TargetedSiteStaticCaptureState = S.Literals([
  'planned',
  'succeeded',
  'partial',
  'failed',
  'blocked',
  'manual_review',
  'archived',
])
export type TargetedSiteStaticCaptureState =
  typeof TargetedSiteStaticCaptureState.Type

export const TargetedSiteStaticCaptureReason = S.Literals([
  'policy_fetchable',
  'policy_not_fetchable',
  'homepage_fetched',
  'partial_pages',
  'network_error',
  'invalid_url',
  'cross_origin_url',
  'response_too_large',
  'unsupported_content_type',
  'robots_changed',
  'manual_review',
  'source_pack_ready',
])
export type TargetedSiteStaticCaptureReason =
  typeof TargetedSiteStaticCaptureReason.Type

export const TargetedSiteStaticAssetKind = S.Literals([
  'image',
  'script',
  'style',
  'font',
  'document',
  'other',
])
export type TargetedSiteStaticAssetKind =
  typeof TargetedSiteStaticAssetKind.Type

export const TargetedSiteStaticPageRef = S.Struct({
  ref: S.String,
  sourceHash: S.NullOr(S.String),
  url: S.String,
})
export type TargetedSiteStaticPageRef = typeof TargetedSiteStaticPageRef.Type

export const TargetedSiteStaticAssetRef = S.Struct({
  kind: TargetedSiteStaticAssetKind,
  ref: S.String,
  sourceHash: S.NullOr(S.String),
  url: S.String,
})
export type TargetedSiteStaticAssetRef = typeof TargetedSiteStaticAssetRef.Type

export const TargetedSiteStaticResponseSummary = S.Struct({
  bytes: S.Number,
  contentType: S.NullOr(S.String),
  headersRef: S.NullOr(S.String),
  status: S.Number,
})
export type TargetedSiteStaticResponseSummary =
  typeof TargetedSiteStaticResponseSummary.Type

export const TargetedSiteStaticCaptureRunRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  assetRefs: S.Array(TargetedSiteStaticAssetRef),
  campaignId: S.String,
  capturePolicyEventId: S.String,
  completedAt: S.NullOr(S.String),
  createdAt: S.String,
  homepageRef: S.NullOr(S.String),
  homepageUrl: S.String,
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  normalizedDomain: S.String,
  pageRefs: S.Array(TargetedSiteStaticPageRef),
  prospectId: S.NullOr(S.String),
  reason: TargetedSiteStaticCaptureReason,
  responseSummary: TargetedSiteStaticResponseSummary,
  robotsRef: S.NullOr(S.String),
  sitemapRef: S.NullOr(S.String),
  sourceHash: S.NullOr(S.String),
  sourcePackRef: S.NullOr(S.String),
  startedAt: S.String,
  state: TargetedSiteStaticCaptureState,
})
export type TargetedSiteStaticCaptureRunRecord =
  typeof TargetedSiteStaticCaptureRunRecord.Type

export const PublicTargetedSiteStaticCaptureProjection = S.Struct({
  assetCount: S.Number,
  campaignId: S.String,
  completedAt: S.NullOr(S.String),
  homepageUrl: S.String,
  normalizedDomain: S.String,
  pageCount: S.Number,
  prospectId: S.NullOr(S.String),
  sourcePackRef: S.NullOr(S.String),
  startedAt: S.String,
  state: TargetedSiteStaticCaptureState,
})
export type PublicTargetedSiteStaticCaptureProjection =
  typeof PublicTargetedSiteStaticCaptureProjection.Type

export const OperatorTargetedSiteStaticCaptureProjection = S.Struct({
  assetCount: S.Number,
  campaignId: S.String,
  capturePolicyEventId: S.String,
  completedAt: S.NullOr(S.String),
  hasMetadata: S.Boolean,
  homepageRef: S.NullOr(S.String),
  homepageUrl: S.String,
  normalizedDomain: S.String,
  pageCount: S.Number,
  prospectId: S.NullOr(S.String),
  reason: TargetedSiteStaticCaptureReason,
  responseSummary: TargetedSiteStaticResponseSummary,
  robotsRef: S.NullOr(S.String),
  sitemapRef: S.NullOr(S.String),
  sourceHash: S.NullOr(S.String),
  sourcePackRef: S.NullOr(S.String),
  startedAt: S.String,
  state: TargetedSiteStaticCaptureState,
})
export type OperatorTargetedSiteStaticCaptureProjection =
  typeof OperatorTargetedSiteStaticCaptureProjection.Type

export type StaticCapturePageInput = Readonly<{
  ref: string
  sourceHash?: string | undefined
  url: string
}>

export type StaticCaptureAssetInput = Readonly<{
  kind: TargetedSiteStaticAssetKind
  ref: string
  sourceHash?: string | undefined
  url: string
}>

export type StaticCaptureResponseSummaryInput = Readonly<{
  bytes?: number | undefined
  contentType?: string | undefined
  headersRef?: string | undefined
  status?: number | undefined
}>

export type RecordTargetedSiteStaticCaptureRunInput = Readonly<{
  assetRefs?: ReadonlyArray<StaticCaptureAssetInput> | undefined
  capturePolicyEvent: TargetedSiteCapturePolicyEventRecord
  completedAt?: string | undefined
  homepageRef?: string | undefined
  homepageUrl?: string | undefined
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  pageRefs?: ReadonlyArray<StaticCapturePageInput> | undefined
  reason?: TargetedSiteStaticCaptureReason | undefined
  responseSummary?: StaticCaptureResponseSummaryInput | undefined
  robotsRef?: string | undefined
  sitemapRef?: string | undefined
  sourceHash?: string | undefined
  sourcePackRef?: string | undefined
  startedAt?: string | undefined
  state?: TargetedSiteStaticCaptureState | undefined
}>

type TargetedSiteStaticCaptureRunRow = Readonly<{
  archived_at: string | null
  asset_refs_json: string
  campaign_id: string
  capture_policy_event_id: string
  completed_at: string | null
  created_at: string
  homepage_ref: string | null
  homepage_url: string
  id: string
  idempotency_key: string
  metadata_json: string
  normalized_domain: string
  page_refs_json: string
  prospect_id: string | null
  reason: TargetedSiteStaticCaptureReason
  response_summary_json: string
  robots_ref: string | null
  sitemap_ref: string | null
  source_hash: string | null
  source_pack_ref: string | null
  started_at: string
  state: TargetedSiteStaticCaptureState
}>

type TargetedSiteStaticCaptureRunFilter =
  | Readonly<{ key: 'campaign_id'; value: string }>
  | Readonly<{ key: 'prospect_id'; value: string }>
  | Readonly<{ key: 'normalized_domain'; value: string }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_DOMAIN_PATTERN =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{1,62}$/
const SAFE_HASH_PATTERN = /^sha256:[a-f0-9]{64}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic|bypass|captcha|cloudflare challenge|headless stealth|anti-bot)|@/i

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const isSafeRef = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value) && textIsSafe(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!isSafeRef(value)) {
    throw new TargetedSiteStaticCaptureValidationError({
      reason: `${field} must be a public-safe ref without raw contact, provider, payment, wallet, or bypass material.`,
    })
  }
}

const assertSafeDomain = (value: string): void => {
  if (!SAFE_DOMAIN_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteStaticCaptureValidationError({
      reason: 'normalizedDomain must be a public-safe normalized domain.',
    })
  }
}

const assertSafeHash = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_HASH_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteStaticCaptureValidationError({
      reason: `${field} must be a sha256 ref without private material.`,
    })
  }
}

const assertSafeText = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!textIsSafe(value)) {
    throw new TargetedSiteStaticCaptureValidationError({
      reason: `${field} must not contain raw contact, provider, payment, wallet, or bypass material.`,
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
    throw new TargetedSiteStaticCaptureValidationError({
      reason:
        'metadata must not contain raw contact, provider, payment, wallet, or bypass material.',
    })
  }
}

const boundedBytes = (value: number | undefined): number => {
  if (value === undefined || !Number.isFinite(value)) {
    return 0
  }

  return Math.max(0, Math.min(20_000_000, Math.floor(value)))
}

const boundedStatus = (value: number | undefined): number => {
  if (value === undefined || !Number.isFinite(value)) {
    return 0
  }

  return Math.max(0, Math.min(599, Math.floor(value)))
}

export class TargetedSiteStaticCaptureValidationError extends S.TaggedErrorClass<TargetedSiteStaticCaptureValidationError>()(
  'TargetedSiteStaticCaptureValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteStaticCaptureStorageError extends S.TaggedErrorClass<TargetedSiteStaticCaptureStorageError>()(
  'TargetedSiteStaticCaptureStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export const normalizeTargetedSiteStaticCaptureUrl = (
  normalizedDomain: string,
  value: string | undefined,
): string => {
  assertSafeDomain(normalizedDomain)

  const candidate =
    value === undefined || value.trim() === ''
      ? `https://${normalizedDomain}/`
      : value.trim()

  if (!textIsSafe(candidate)) {
    throw new TargetedSiteStaticCaptureValidationError({
      reason: 'capture URL must not contain private material.',
    })
  }

  try {
    const url = new URL(candidate, `https://${normalizedDomain}/`)

    if (url.protocol !== 'https:' && url.protocol !== 'http:') {
      throw new TargetedSiteStaticCaptureValidationError({
        reason: 'capture URL must use http or https.',
      })
    }

    if (url.username !== '' || url.password !== '') {
      throw new TargetedSiteStaticCaptureValidationError({
        reason: 'capture URL must not include credentials.',
      })
    }

    const hostname = url.hostname.toLowerCase().replace(/^www\./, '')

    if (hostname !== normalizedDomain) {
      throw new TargetedSiteStaticCaptureValidationError({
        reason: 'capture URL must stay on the normalized domain.',
      })
    }

    url.hash = ''

    return url.toString()
  } catch (error) {
    if (error instanceof TargetedSiteStaticCaptureValidationError) {
      throw error
    }

    throw new TargetedSiteStaticCaptureValidationError({
      reason: 'capture URL must be a valid same-origin http(s) URL.',
    })
  }
}

const normalizePageRefs = (
  normalizedDomain: string,
  pageRefs: ReadonlyArray<StaticCapturePageInput> | undefined,
): ReadonlyArray<TargetedSiteStaticPageRef> =>
  (pageRefs ?? []).slice(0, 25).map(pageRef => {
    assertSafeRef('pageRef.ref', pageRef.ref)
    assertSafeHash('pageRef.sourceHash', pageRef.sourceHash)

    return {
      ref: pageRef.ref,
      sourceHash: pageRef.sourceHash ?? null,
      url: normalizeTargetedSiteStaticCaptureUrl(normalizedDomain, pageRef.url),
    }
  })

const normalizeAssetRefs = (
  normalizedDomain: string,
  assetRefs: ReadonlyArray<StaticCaptureAssetInput> | undefined,
): ReadonlyArray<TargetedSiteStaticAssetRef> =>
  (assetRefs ?? []).slice(0, 100).map(assetRef => {
    assertSafeRef('assetRef.ref', assetRef.ref)
    assertSafeHash('assetRef.sourceHash', assetRef.sourceHash)

    return {
      kind: assetRef.kind,
      ref: assetRef.ref,
      sourceHash: assetRef.sourceHash ?? null,
      url: normalizeTargetedSiteStaticCaptureUrl(normalizedDomain, assetRef.url),
    }
  })

const normalizeResponseSummary = (
  responseSummary: StaticCaptureResponseSummaryInput | undefined,
): TargetedSiteStaticResponseSummary => {
  assertSafeRef('responseSummary.headersRef', responseSummary?.headersRef)
  assertSafeText('responseSummary.contentType', responseSummary?.contentType)

  return {
    bytes: boundedBytes(responseSummary?.bytes),
    contentType: responseSummary?.contentType?.trim().slice(0, 120) ?? null,
    headersRef: responseSummary?.headersRef ?? null,
    status: boundedStatus(responseSummary?.status),
  }
}

const assertFetchablePolicy = (
  policy: TargetedSiteCapturePolicyEventRecord,
): void => {
  assertSafeDomain(policy.normalizedDomain)

  if (!isTargetedSiteCaptureFetchable(policy)) {
    throw new TargetedSiteStaticCaptureValidationError({
      reason:
        'static capture requires an explicit allowed or paid-escalation capture policy event.',
    })
  }
}

const assertValidInput = (
  input: RecordTargetedSiteStaticCaptureRunInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('homepageRef', input.homepageRef)
  assertSafeRef('robotsRef', input.robotsRef)
  assertSafeRef('sitemapRef', input.sitemapRef)
  assertSafeRef('sourcePackRef', input.sourcePackRef)
  assertSafeHash('sourceHash', input.sourceHash)
  assertSafeMetadata(input.metadata)
  assertFetchablePolicy(input.capturePolicyEvent)
}

const stateAndReason = (
  input: RecordTargetedSiteStaticCaptureRunInput,
): readonly [TargetedSiteStaticCaptureState, TargetedSiteStaticCaptureReason] => {
  if (input.state !== undefined && input.reason !== undefined) {
    return [input.state, input.reason]
  }

  if (input.sourcePackRef !== undefined) {
    return [input.state ?? 'succeeded', input.reason ?? 'source_pack_ready']
  }

  if (input.homepageRef !== undefined) {
    return [input.state ?? 'succeeded', input.reason ?? 'homepage_fetched']
  }

  return [input.state ?? 'planned', input.reason ?? 'policy_fetchable']
}

const metadataFromJson = (json: string): Readonly<Record<string, unknown>> =>
  parseJsonRecord(json) ?? {}

const parsePageRefs = (json: string): ReadonlyArray<TargetedSiteStaticPageRef> =>
  parseJsonRecord(`{"items":${json}}`)?.items instanceof Array
    ? (parseJsonRecord(`{"items":${json}}`)?.items as Array<Record<string, unknown>>)
        .filter(item => typeof item.ref === 'string' && typeof item.url === 'string')
        .map(item => ({
          ref: String(item.ref),
          sourceHash:
            typeof item.sourceHash === 'string' ? String(item.sourceHash) : null,
          url: String(item.url),
        }))
    : []

const parseAssetRefs = (
  json: string,
): ReadonlyArray<TargetedSiteStaticAssetRef> =>
  parseJsonRecord(`{"items":${json}}`)?.items instanceof Array
    ? (parseJsonRecord(`{"items":${json}}`)?.items as Array<Record<string, unknown>>)
        .filter(item => typeof item.ref === 'string' && typeof item.url === 'string')
        .map(item => ({
          kind:
            item.kind === 'image' ||
            item.kind === 'script' ||
            item.kind === 'style' ||
            item.kind === 'font' ||
            item.kind === 'document'
              ? item.kind
              : 'other',
          ref: String(item.ref),
          sourceHash:
            typeof item.sourceHash === 'string' ? String(item.sourceHash) : null,
          url: String(item.url),
        }))
    : []

const responseSummaryFromJson = (
  json: string,
): TargetedSiteStaticResponseSummary => {
  const parsed = parseJsonRecord(json) ?? {}

  return {
    bytes: typeof parsed.bytes === 'number' ? boundedBytes(parsed.bytes) : 0,
    contentType:
      typeof parsed.contentType === 'string' ? parsed.contentType : null,
    headersRef: typeof parsed.headersRef === 'string' ? parsed.headersRef : null,
    status: typeof parsed.status === 'number' ? boundedStatus(parsed.status) : 0,
  }
}

const runFromRow = (
  row: TargetedSiteStaticCaptureRunRow,
): TargetedSiteStaticCaptureRunRecord => ({
  archivedAt: row.archived_at,
  assetRefs: [...parseAssetRefs(row.asset_refs_json)],
  campaignId: row.campaign_id,
  capturePolicyEventId: row.capture_policy_event_id,
  completedAt: row.completed_at,
  createdAt: row.created_at,
  homepageRef: row.homepage_ref,
  homepageUrl: row.homepage_url,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: metadataFromJson(row.metadata_json),
  normalizedDomain: row.normalized_domain,
  pageRefs: [...parsePageRefs(row.page_refs_json)],
  prospectId: row.prospect_id,
  reason: row.reason,
  responseSummary: responseSummaryFromJson(row.response_summary_json),
  robotsRef: row.robots_ref,
  sitemapRef: row.sitemap_ref,
  sourceHash: row.source_hash,
  sourcePackRef: row.source_pack_ref,
  startedAt: row.started_at,
  state: row.state,
})

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<TargetedSiteStaticCaptureRunRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM targeted_site_static_capture_runs
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<TargetedSiteStaticCaptureRunRow>()

  return row === null ? null : runFromRow(row)
}

const listByFilter = async (
  db: D1Database,
  filter: TargetedSiteStaticCaptureRunFilter,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteStaticCaptureRunRecord>> => {
  if (filter.key === 'normalized_domain') {
    assertSafeDomain(filter.value)
  } else {
    assertSafeRef(filter.key, filter.value)
  }

  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM targeted_site_static_capture_runs
        WHERE ${filter.key} = ?
          AND archived_at IS NULL
        ORDER BY started_at DESC
        LIMIT ?`,
    )
    .bind(filter.value, safeLimit)
    .all<TargetedSiteStaticCaptureRunRow>()

  return (rows.results ?? []).map(runFromRow)
}

export const publicTargetedSiteStaticCaptureProjection = (
  record: TargetedSiteStaticCaptureRunRecord,
): PublicTargetedSiteStaticCaptureProjection => ({
  assetCount: record.assetRefs.length,
  campaignId: record.campaignId,
  completedAt: record.completedAt,
  homepageUrl: record.homepageUrl,
  normalizedDomain: record.normalizedDomain,
  pageCount: record.pageRefs.length,
  prospectId: record.prospectId,
  sourcePackRef: record.sourcePackRef,
  startedAt: record.startedAt,
  state: record.state,
})

export const operatorTargetedSiteStaticCaptureProjection = (
  record: TargetedSiteStaticCaptureRunRecord,
): OperatorTargetedSiteStaticCaptureProjection => ({
  assetCount: record.assetRefs.length,
  campaignId: record.campaignId,
  capturePolicyEventId: record.capturePolicyEventId,
  completedAt: record.completedAt,
  hasMetadata: Object.keys(record.metadata).length > 0,
  homepageRef: record.homepageRef,
  homepageUrl: record.homepageUrl,
  normalizedDomain: record.normalizedDomain,
  pageCount: record.pageRefs.length,
  prospectId: record.prospectId,
  reason: record.reason,
  responseSummary: record.responseSummary,
  robotsRef: record.robotsRef,
  sitemapRef: record.sitemapRef,
  sourceHash: record.sourceHash,
  sourcePackRef: record.sourcePackRef,
  startedAt: record.startedAt,
  state: record.state,
})

export const recordTargetedSiteStaticCaptureRun = async (
  db: D1Database,
  input: RecordTargetedSiteStaticCaptureRunInput,
): Promise<TargetedSiteStaticCaptureRunRecord> => {
  assertValidInput(input)

  const existing = await readByIdempotencyKey(db, input.idempotencyKey)

  if (existing !== null) {
    return existing
  }

  const nowIso = currentIsoTimestamp()
  const startedAt = input.startedAt ?? nowIso
  const completedAt = input.completedAt ?? null
  const policy = input.capturePolicyEvent
  const homepageUrl = normalizeTargetedSiteStaticCaptureUrl(
    policy.normalizedDomain,
    input.homepageUrl,
  )
  const pageRefs = normalizePageRefs(policy.normalizedDomain, input.pageRefs)
  const assetRefs = normalizeAssetRefs(policy.normalizedDomain, input.assetRefs)
  const responseSummary = normalizeResponseSummary(input.responseSummary)
  const [state, reason] = stateAndReason(input)
  const id = input.id ?? compactRandomId('targeted_site_static_capture')

  await db
    .prepare(
      `INSERT OR IGNORE INTO targeted_site_static_capture_runs (
         id,
         idempotency_key,
         campaign_id,
         prospect_id,
         normalized_domain,
         capture_policy_event_id,
         state,
         reason,
         homepage_url,
         homepage_ref,
         robots_ref,
         sitemap_ref,
         source_pack_ref,
         source_hash,
         page_refs_json,
         asset_refs_json,
         response_summary_json,
         metadata_json,
         started_at,
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
      state,
      reason,
      homepageUrl,
      input.homepageRef ?? null,
      input.robotsRef ?? policy.robotsRef,
      input.sitemapRef ?? policy.sitemapRef,
      input.sourcePackRef ?? null,
      input.sourceHash ?? null,
      JSON.stringify(pageRefs),
      JSON.stringify(assetRefs),
      JSON.stringify(responseSummary),
      JSON.stringify(input.metadata ?? {}),
      startedAt,
      completedAt,
      nowIso,
    )
    .run()

  const record = await readByIdempotencyKey(db, input.idempotencyKey)

  if (record === null) {
    throw new TargetedSiteStaticCaptureStorageError({
      operation: 'recordTargetedSiteStaticCaptureRun.readByIdempotencyKey',
      reason: 'inserted or existing targeted Site static capture run was not readable.',
    })
  }

  return record
}

export const listTargetedSiteStaticCaptureRunsByCampaign = async (
  db: D1Database,
  campaignId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteStaticCaptureRunRecord>> =>
  listByFilter(db, { key: 'campaign_id', value: campaignId }, limit)

export const listTargetedSiteStaticCaptureRunsByProspect = async (
  db: D1Database,
  prospectId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteStaticCaptureRunRecord>> =>
  listByFilter(db, { key: 'prospect_id', value: prospectId }, limit)

export const listTargetedSiteStaticCaptureRunsByDomain = async (
  db: D1Database,
  normalizedDomain: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteStaticCaptureRunRecord>> =>
  listByFilter(db, { key: 'normalized_domain', value: normalizedDomain }, limit)
