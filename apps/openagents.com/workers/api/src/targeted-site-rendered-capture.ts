import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  isTargetedSiteCaptureFetchable,
  type TargetedSiteCapturePolicyEventRecord,
} from './targeted-site-capture-policy'
import { normalizeTargetedSiteStaticCaptureUrl } from './targeted-site-static-capture'

export const TargetedSiteRenderedCaptureState = S.Literals([
  'planned',
  'succeeded',
  'partial',
  'failed',
  'blocked',
  'manual_review',
  'archived',
])
export type TargetedSiteRenderedCaptureState =
  typeof TargetedSiteRenderedCaptureState.Type

export const TargetedSiteRenderedCaptureReason = S.Literals([
  'policy_fetchable',
  'policy_not_fetchable',
  'static_capture_insufficient',
  'screenshot_ready',
  'rendered_source_ready',
  'crawl_ready',
  'usage_limit',
  'network_error',
  'provider_error',
  'bot_protection_or_login',
  'manual_review',
])
export type TargetedSiteRenderedCaptureReason =
  typeof TargetedSiteRenderedCaptureReason.Type

export const TargetedSiteRenderedUsageSummary = S.Struct({
  browserMs: S.Number,
  bytes: S.Number,
  costRef: S.NullOr(S.String),
  estimatedCostCredits: S.Number,
  pages: S.Number,
})
export type TargetedSiteRenderedUsageSummary =
  typeof TargetedSiteRenderedUsageSummary.Type

export const TargetedSiteRenderedCaptureRunRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  campaignId: S.String,
  capturePolicyEventId: S.String,
  completedAt: S.NullOr(S.String),
  crawlRef: S.NullOr(S.String),
  createdAt: S.String,
  deviceRef: S.NullOr(S.String),
  id: S.String,
  idempotencyKey: S.String,
  linksRef: S.NullOr(S.String),
  markdownRef: S.NullOr(S.String),
  metadata: S.Record(S.String, S.Unknown),
  normalizedDomain: S.String,
  prospectId: S.NullOr(S.String),
  providerRef: S.String,
  reason: TargetedSiteRenderedCaptureReason,
  renderedHtmlRef: S.NullOr(S.String),
  screenshotRef: S.NullOr(S.String),
  startedAt: S.String,
  state: TargetedSiteRenderedCaptureState,
  staticCaptureRunId: S.NullOr(S.String),
  structuredJsonRef: S.NullOr(S.String),
  targetUrl: S.String,
  usageSummary: TargetedSiteRenderedUsageSummary,
  viewportRef: S.NullOr(S.String),
})
export type TargetedSiteRenderedCaptureRunRecord =
  typeof TargetedSiteRenderedCaptureRunRecord.Type

export const PublicTargetedSiteRenderedCaptureProjection = S.Struct({
  campaignId: S.String,
  completedAt: S.NullOr(S.String),
  hasCrawl: S.Boolean,
  hasMarkdown: S.Boolean,
  hasScreenshot: S.Boolean,
  normalizedDomain: S.String,
  prospectId: S.NullOr(S.String),
  startedAt: S.String,
  state: TargetedSiteRenderedCaptureState,
  targetUrl: S.String,
})
export type PublicTargetedSiteRenderedCaptureProjection =
  typeof PublicTargetedSiteRenderedCaptureProjection.Type

export const OperatorTargetedSiteRenderedCaptureProjection = S.Struct({
  campaignId: S.String,
  capturePolicyEventId: S.String,
  completedAt: S.NullOr(S.String),
  crawlRef: S.NullOr(S.String),
  deviceRef: S.NullOr(S.String),
  hasMetadata: S.Boolean,
  linksRef: S.NullOr(S.String),
  markdownRef: S.NullOr(S.String),
  normalizedDomain: S.String,
  prospectId: S.NullOr(S.String),
  providerRef: S.String,
  reason: TargetedSiteRenderedCaptureReason,
  renderedHtmlRef: S.NullOr(S.String),
  screenshotRef: S.NullOr(S.String),
  startedAt: S.String,
  state: TargetedSiteRenderedCaptureState,
  staticCaptureRunId: S.NullOr(S.String),
  structuredJsonRef: S.NullOr(S.String),
  targetUrl: S.String,
  usageSummary: TargetedSiteRenderedUsageSummary,
  viewportRef: S.NullOr(S.String),
})
export type OperatorTargetedSiteRenderedCaptureProjection =
  typeof OperatorTargetedSiteRenderedCaptureProjection.Type

export type RenderedCaptureUsageInput = Readonly<{
  browserMs?: number | undefined
  bytes?: number | undefined
  costRef?: string | undefined
  estimatedCostCredits?: number | undefined
  pages?: number | undefined
}>

export type RecordTargetedSiteRenderedCaptureRunInput = Readonly<{
  capturePolicyEvent: TargetedSiteCapturePolicyEventRecord
  completedAt?: string | undefined
  crawlRef?: string | undefined
  deviceRef?: string | undefined
  id?: string | undefined
  idempotencyKey: string
  linksRef?: string | undefined
  markdownRef?: string | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  providerRef?: string | undefined
  reason?: TargetedSiteRenderedCaptureReason | undefined
  renderedHtmlRef?: string | undefined
  screenshotRef?: string | undefined
  signals?: Readonly<{
    botProtectionOrLogin?: boolean | undefined
    manualReviewRequested?: boolean | undefined
  }> | undefined
  startedAt?: string | undefined
  state?: TargetedSiteRenderedCaptureState | undefined
  staticCaptureRunId?: string | undefined
  structuredJsonRef?: string | undefined
  targetUrl?: string | undefined
  usageSummary?: RenderedCaptureUsageInput | undefined
  viewportRef?: string | undefined
}>

type TargetedSiteRenderedCaptureRunRow = Readonly<{
  archived_at: string | null
  campaign_id: string
  capture_policy_event_id: string
  completed_at: string | null
  crawl_ref: string | null
  created_at: string
  device_ref: string | null
  id: string
  idempotency_key: string
  links_ref: string | null
  markdown_ref: string | null
  metadata_json: string
  normalized_domain: string
  prospect_id: string | null
  provider_ref: string
  reason: TargetedSiteRenderedCaptureReason
  rendered_html_ref: string | null
  screenshot_ref: string | null
  started_at: string
  state: TargetedSiteRenderedCaptureState
  static_capture_run_id: string | null
  structured_json_ref: string | null
  target_url: string
  usage_summary_json: string
  viewport_ref: string | null
}>

type TargetedSiteRenderedCaptureRunFilter =
  | Readonly<{ key: 'campaign_id'; value: string }>
  | Readonly<{ key: 'prospect_id'; value: string }>
  | Readonly<{ key: 'normalized_domain'; value: string }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_DOMAIN_PATTERN =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{1,62}$/
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
    throw new TargetedSiteRenderedCaptureValidationError({
      reason: `${field} must be a public-safe ref without raw provider, browser-log, contact, payment, wallet, or bypass material.`,
    })
  }
}

const assertSafeDomain = (value: string): void => {
  if (!SAFE_DOMAIN_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteRenderedCaptureValidationError({
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
    throw new TargetedSiteRenderedCaptureValidationError({
      reason:
        'metadata must not contain raw provider, browser-log, contact, payment, wallet, or bypass material.',
    })
  }
}

const boundedNumber = (
  value: number | undefined,
  max: number,
): number => {
  if (value === undefined || !Number.isFinite(value)) {
    return 0
  }

  return Math.max(0, Math.min(max, Math.floor(value)))
}

const boundedCredits = (value: number | undefined): number => {
  if (value === undefined || !Number.isFinite(value)) {
    return 0
  }

  return Math.max(0, Math.min(10_000, Number(value.toFixed(4))))
}

const usageSummary = (
  input: RenderedCaptureUsageInput | undefined,
): TargetedSiteRenderedUsageSummary => {
  assertSafeRef('usageSummary.costRef', input?.costRef)

  return {
    browserMs: boundedNumber(input?.browserMs, 3_600_000),
    bytes: boundedNumber(input?.bytes, 50_000_000),
    costRef: input?.costRef ?? null,
    estimatedCostCredits: boundedCredits(input?.estimatedCostCredits),
    pages: boundedNumber(input?.pages, 100),
  }
}

const metadataFromJson = (json: string): Readonly<Record<string, unknown>> =>
  parseJsonRecord(json) ?? {}

const usageSummaryFromJson = (
  json: string,
): TargetedSiteRenderedUsageSummary => {
  const parsed = parseJsonRecord(json) ?? {}

  return {
    browserMs:
      typeof parsed.browserMs === 'number' ? boundedNumber(parsed.browserMs, 3_600_000) : 0,
    bytes: typeof parsed.bytes === 'number' ? boundedNumber(parsed.bytes, 50_000_000) : 0,
    costRef: typeof parsed.costRef === 'string' ? parsed.costRef : null,
    estimatedCostCredits:
      typeof parsed.estimatedCostCredits === 'number'
        ? boundedCredits(parsed.estimatedCostCredits)
        : 0,
    pages: typeof parsed.pages === 'number' ? boundedNumber(parsed.pages, 100) : 0,
  }
}

const assertFetchablePolicy = (
  policy: TargetedSiteCapturePolicyEventRecord,
): void => {
  assertSafeDomain(policy.normalizedDomain)

  if (!isTargetedSiteCaptureFetchable(policy)) {
    throw new TargetedSiteRenderedCaptureValidationError({
      reason:
        'rendered capture requires an explicit allowed or paid-escalation capture policy event.',
    })
  }
}

const hasRenderedOutput = (
  input: RecordTargetedSiteRenderedCaptureRunInput,
): boolean =>
  input.screenshotRef !== undefined ||
  input.renderedHtmlRef !== undefined ||
  input.markdownRef !== undefined ||
  input.linksRef !== undefined ||
  input.structuredJsonRef !== undefined ||
  input.crawlRef !== undefined

const stateAndReason = (
  input: RecordTargetedSiteRenderedCaptureRunInput,
): readonly [TargetedSiteRenderedCaptureState, TargetedSiteRenderedCaptureReason] => {
  if (input.signals?.botProtectionOrLogin === true) {
    return ['blocked', 'bot_protection_or_login']
  }

  if (input.signals?.manualReviewRequested === true) {
    return ['manual_review', 'manual_review']
  }

  if (input.state !== undefined && input.reason !== undefined) {
    return [input.state, input.reason]
  }

  if (input.crawlRef !== undefined) {
    return [input.state ?? 'succeeded', input.reason ?? 'crawl_ready']
  }

  if (input.renderedHtmlRef !== undefined || input.markdownRef !== undefined) {
    return [input.state ?? 'succeeded', input.reason ?? 'rendered_source_ready']
  }

  if (input.screenshotRef !== undefined) {
    return [input.state ?? 'succeeded', input.reason ?? 'screenshot_ready']
  }

  if (input.staticCaptureRunId !== undefined) {
    return [
      input.state ?? 'planned',
      input.reason ?? 'static_capture_insufficient',
    ]
  }

  return [input.state ?? 'planned', input.reason ?? 'policy_fetchable']
}

const assertValidInput = (
  input: RecordTargetedSiteRenderedCaptureRunInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('providerRef', input.providerRef)
  assertSafeRef('staticCaptureRunId', input.staticCaptureRunId)
  assertSafeRef('screenshotRef', input.screenshotRef)
  assertSafeRef('renderedHtmlRef', input.renderedHtmlRef)
  assertSafeRef('markdownRef', input.markdownRef)
  assertSafeRef('linksRef', input.linksRef)
  assertSafeRef('structuredJsonRef', input.structuredJsonRef)
  assertSafeRef('crawlRef', input.crawlRef)
  assertSafeRef('viewportRef', input.viewportRef)
  assertSafeRef('deviceRef', input.deviceRef)
  assertSafeMetadata(input.metadata)
  assertFetchablePolicy(input.capturePolicyEvent)

  if (
    input.signals?.botProtectionOrLogin === true &&
    hasRenderedOutput(input)
  ) {
    throw new TargetedSiteRenderedCaptureValidationError({
      reason: 'bot-protection or login-wall targets cannot record rendered output refs.',
    })
  }
}

export class TargetedSiteRenderedCaptureValidationError extends S.TaggedErrorClass<TargetedSiteRenderedCaptureValidationError>()(
  'TargetedSiteRenderedCaptureValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteRenderedCaptureStorageError extends S.TaggedErrorClass<TargetedSiteRenderedCaptureStorageError>()(
  'TargetedSiteRenderedCaptureStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

const runFromRow = (
  row: TargetedSiteRenderedCaptureRunRow,
): TargetedSiteRenderedCaptureRunRecord => ({
  archivedAt: row.archived_at,
  campaignId: row.campaign_id,
  capturePolicyEventId: row.capture_policy_event_id,
  completedAt: row.completed_at,
  crawlRef: row.crawl_ref,
  createdAt: row.created_at,
  deviceRef: row.device_ref,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  linksRef: row.links_ref,
  markdownRef: row.markdown_ref,
  metadata: metadataFromJson(row.metadata_json),
  normalizedDomain: row.normalized_domain,
  prospectId: row.prospect_id,
  providerRef: row.provider_ref,
  reason: row.reason,
  renderedHtmlRef: row.rendered_html_ref,
  screenshotRef: row.screenshot_ref,
  startedAt: row.started_at,
  state: row.state,
  staticCaptureRunId: row.static_capture_run_id,
  structuredJsonRef: row.structured_json_ref,
  targetUrl: row.target_url,
  usageSummary: usageSummaryFromJson(row.usage_summary_json),
  viewportRef: row.viewport_ref,
})

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<TargetedSiteRenderedCaptureRunRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM targeted_site_rendered_capture_runs
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<TargetedSiteRenderedCaptureRunRow>()

  return row === null ? null : runFromRow(row)
}

const listByFilter = async (
  db: D1Database,
  filter: TargetedSiteRenderedCaptureRunFilter,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteRenderedCaptureRunRecord>> => {
  if (filter.key === 'normalized_domain') {
    assertSafeDomain(filter.value)
  } else {
    assertSafeRef(filter.key, filter.value)
  }

  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM targeted_site_rendered_capture_runs
        WHERE ${filter.key} = ?
          AND archived_at IS NULL
        ORDER BY started_at DESC
        LIMIT ?`,
    )
    .bind(filter.value, safeLimit)
    .all<TargetedSiteRenderedCaptureRunRow>()

  return (rows.results ?? []).map(runFromRow)
}

export const publicTargetedSiteRenderedCaptureProjection = (
  record: TargetedSiteRenderedCaptureRunRecord,
): PublicTargetedSiteRenderedCaptureProjection => ({
  campaignId: record.campaignId,
  completedAt: record.completedAt,
  hasCrawl: record.crawlRef !== null,
  hasMarkdown: record.markdownRef !== null,
  hasScreenshot: record.screenshotRef !== null,
  normalizedDomain: record.normalizedDomain,
  prospectId: record.prospectId,
  startedAt: record.startedAt,
  state: record.state,
  targetUrl: record.targetUrl,
})

export const operatorTargetedSiteRenderedCaptureProjection = (
  record: TargetedSiteRenderedCaptureRunRecord,
): OperatorTargetedSiteRenderedCaptureProjection => ({
  campaignId: record.campaignId,
  capturePolicyEventId: record.capturePolicyEventId,
  completedAt: record.completedAt,
  crawlRef: record.crawlRef,
  deviceRef: record.deviceRef,
  hasMetadata: Object.keys(record.metadata).length > 0,
  linksRef: record.linksRef,
  markdownRef: record.markdownRef,
  normalizedDomain: record.normalizedDomain,
  prospectId: record.prospectId,
  providerRef: record.providerRef,
  reason: record.reason,
  renderedHtmlRef: record.renderedHtmlRef,
  screenshotRef: record.screenshotRef,
  startedAt: record.startedAt,
  state: record.state,
  staticCaptureRunId: record.staticCaptureRunId,
  structuredJsonRef: record.structuredJsonRef,
  targetUrl: record.targetUrl,
  usageSummary: record.usageSummary,
  viewportRef: record.viewportRef,
})

export const recordTargetedSiteRenderedCaptureRun = async (
  db: D1Database,
  input: RecordTargetedSiteRenderedCaptureRunInput,
): Promise<TargetedSiteRenderedCaptureRunRecord> => {
  assertValidInput(input)

  const existing = await readByIdempotencyKey(db, input.idempotencyKey)

  if (existing !== null) {
    return existing
  }

  const nowIso = currentIsoTimestamp()
  const policy = input.capturePolicyEvent
  const startedAt = input.startedAt ?? nowIso
  const [state, reason] = stateAndReason(input)
  const id = input.id ?? compactRandomId('targeted_site_rendered_capture')
  const targetUrl = normalizeTargetedSiteStaticCaptureUrl(
    policy.normalizedDomain,
    input.targetUrl,
  )
  const usage = usageSummary(input.usageSummary)

  await db
    .prepare(
      `INSERT OR IGNORE INTO targeted_site_rendered_capture_runs (
         id,
         idempotency_key,
         campaign_id,
         prospect_id,
         normalized_domain,
         capture_policy_event_id,
         static_capture_run_id,
         state,
         reason,
         target_url,
         provider_ref,
         screenshot_ref,
         rendered_html_ref,
         markdown_ref,
         links_ref,
         structured_json_ref,
         crawl_ref,
         viewport_ref,
         device_ref,
         usage_summary_json,
         metadata_json,
         started_at,
         completed_at,
         created_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .bind(
      id,
      input.idempotencyKey,
      policy.campaignId,
      policy.prospectId,
      policy.normalizedDomain,
      policy.id,
      input.staticCaptureRunId ?? null,
      state,
      reason,
      targetUrl,
      input.providerRef ?? 'browser_run',
      input.screenshotRef ?? null,
      input.renderedHtmlRef ?? null,
      input.markdownRef ?? null,
      input.linksRef ?? null,
      input.structuredJsonRef ?? null,
      input.crawlRef ?? null,
      input.viewportRef ?? null,
      input.deviceRef ?? null,
      JSON.stringify(usage),
      JSON.stringify(input.metadata ?? {}),
      startedAt,
      input.completedAt ?? null,
      nowIso,
    )
    .run()

  const record = await readByIdempotencyKey(db, input.idempotencyKey)

  if (record === null) {
    throw new TargetedSiteRenderedCaptureStorageError({
      operation: 'recordTargetedSiteRenderedCaptureRun.readByIdempotencyKey',
      reason: 'inserted or existing targeted Site rendered capture run was not readable.',
    })
  }

  return record
}

export const listTargetedSiteRenderedCaptureRunsByCampaign = async (
  db: D1Database,
  campaignId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteRenderedCaptureRunRecord>> =>
  listByFilter(db, { key: 'campaign_id', value: campaignId }, limit)

export const listTargetedSiteRenderedCaptureRunsByProspect = async (
  db: D1Database,
  prospectId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteRenderedCaptureRunRecord>> =>
  listByFilter(db, { key: 'prospect_id', value: prospectId }, limit)

export const listTargetedSiteRenderedCaptureRunsByDomain = async (
  db: D1Database,
  normalizedDomain: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteRenderedCaptureRunRecord>> =>
  listByFilter(db, { key: 'normalized_domain', value: normalizedDomain }, limit)
