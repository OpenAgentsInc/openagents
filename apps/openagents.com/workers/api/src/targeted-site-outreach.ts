import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import { parseJsonRecord, parseJsonStringArray } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const TargetedSiteCampaignOperatorState = S.Literals([
  'draft',
  'active',
  'paused',
  'reviewing',
  'completed',
  'archived',
])
export type TargetedSiteCampaignOperatorState =
  typeof TargetedSiteCampaignOperatorState.Type

export const TargetedSiteProspectSuppressionState = S.Literals([
  'unknown',
  'clear',
  'suppressed',
  'manual_review',
])
export type TargetedSiteProspectSuppressionState =
  typeof TargetedSiteProspectSuppressionState.Type

export const TargetedSiteProspectCaptureState = S.Literals([
  'not_started',
  'policy_pending',
  'allowed',
  'blocked',
  'captured',
  'archived',
])
export type TargetedSiteProspectCaptureState =
  typeof TargetedSiteProspectCaptureState.Type

export const TargetedSiteProspectReviewState = S.Literals([
  'pending',
  'ready',
  'approved',
  'skipped',
  'archived',
])
export type TargetedSiteProspectReviewState =
  typeof TargetedSiteProspectReviewState.Type

export const TargetedSiteCampaignRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  budgetCapRef: S.NullOr(S.String),
  createdAt: S.String,
  geography: S.NullOr(S.String),
  id: S.String,
  metadata: S.Record(S.String, S.Unknown),
  name: S.String,
  operatorState: TargetedSiteCampaignOperatorState,
  operatorUserId: S.NullOr(S.String),
  ownerUserId: S.String,
  slug: S.String,
  sourceAuthorityRef: S.String,
  suppressionPolicyRef: S.NullOr(S.String),
  updatedAt: S.String,
  vertical: S.NullOr(S.String),
})
export type TargetedSiteCampaignRecord =
  typeof TargetedSiteCampaignRecord.Type

export const TargetedSiteProspectRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  campaignId: S.String,
  captureState: TargetedSiteProspectCaptureState,
  companyName: S.NullOr(S.String),
  contactRefs: S.Array(S.String),
  createdAt: S.String,
  discoveredAt: S.String,
  discoveryConfidence: S.Number,
  geography: S.NullOr(S.String),
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  normalizedDomain: S.String,
  originUrl: S.NullOr(S.String),
  reviewState: TargetedSiteProspectReviewState,
  siteName: S.NullOr(S.String),
  sourceRef: S.String,
  suppressionState: TargetedSiteProspectSuppressionState,
  updatedAt: S.String,
  vertical: S.NullOr(S.String),
})
export type TargetedSiteProspectRecord =
  typeof TargetedSiteProspectRecord.Type

export type CreateTargetedSiteCampaignInput = Readonly<{
  budgetCapRef?: string | undefined
  geography?: string | undefined
  id?: string | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  name: string
  operatorState?: TargetedSiteCampaignOperatorState | undefined
  operatorUserId?: string | undefined
  ownerUserId: string
  slug: string
  sourceAuthorityRef: string
  suppressionPolicyRef?: string | undefined
  vertical?: string | undefined
}>

export type UpsertTargetedSiteProspectInput = Readonly<{
  campaignId: string
  captureState?: TargetedSiteProspectCaptureState | undefined
  companyName?: string | undefined
  contactRefs?: ReadonlyArray<string> | undefined
  discoveryConfidence: number
  geography?: string | undefined
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  originUrl?: string | undefined
  reviewState?: TargetedSiteProspectReviewState | undefined
  siteName?: string | undefined
  sourceRef: string
  suppressionState?: TargetedSiteProspectSuppressionState | undefined
  targetDomain: string
  vertical?: string | undefined
}>

type TargetedSiteCampaignRow = Readonly<{
  archived_at: string | null
  budget_cap_ref: string | null
  created_at: string
  geography: string | null
  id: string
  metadata_json: string
  name: string
  operator_state: TargetedSiteCampaignOperatorState
  operator_user_id: string | null
  owner_user_id: string
  slug: string
  source_authority_ref: string
  suppression_policy_ref: string | null
  updated_at: string
  vertical: string | null
}>

type TargetedSiteProspectRow = Readonly<{
  archived_at: string | null
  campaign_id: string
  capture_state: TargetedSiteProspectCaptureState
  company_name: string | null
  contact_refs_json: string
  created_at: string
  discovered_at: string
  discovery_confidence: number
  geography: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  normalized_domain: string
  origin_url: string | null
  review_state: TargetedSiteProspectReviewState
  site_name: string | null
  source_ref: string
  suppression_state: TargetedSiteProspectSuppressionState
  updated_at: string
  vertical: string | null
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_SLUG_PATTERN = /^[a-z0-9][a-z0-9-]{0,120}$/
const SAFE_DOMAIN_PATTERN =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{1,62}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic)\b|@/i

const compactText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const optionalText = (
  value: string | undefined,
  maxLength: number,
): string | undefined => {
  if (value === undefined) {
    return undefined
  }

  const compact = compactText(value, maxLength)

  return compact === '' ? undefined : compact
}

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const isSafeRef = (value: string): boolean =>
  SAFE_REF_PATTERN.test(value) && textIsSafe(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!isSafeRef(value)) {
    throw new TargetedSiteOutreachValidationError({
      reason: `${field} must be a public-safe ref without raw contact, provider, payment, wallet, or operator-note material.`,
    })
  }
}

const assertSafeText = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!textIsSafe(value)) {
    throw new TargetedSiteOutreachValidationError({
      reason: `${field} must not contain raw contact, provider, payment, wallet, or operator-note material.`,
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
    throw new TargetedSiteOutreachValidationError({
      reason:
        'metadata must not contain raw contact, provider, payment, wallet, or operator-note material.',
    })
  }
}

const normalizeDomain = (value: string): string => {
  const trimmed = value.trim().toLowerCase()
  const candidate = /^[a-z][a-z0-9+.-]*:\/\//i.test(trimmed)
    ? trimmed
    : `https://${trimmed}`

  try {
    const url = new URL(candidate)

    if (url.protocol !== 'https:' && url.protocol !== 'http:') {
      throw new TargetedSiteOutreachValidationError({
        reason: 'targetDomain must be an http(s) domain.',
      })
    }

    const hostname = url.hostname.replace(/^www\./, '')

    if (!SAFE_DOMAIN_PATTERN.test(hostname) || !textIsSafe(hostname)) {
      throw new TargetedSiteOutreachValidationError({
        reason: 'targetDomain must be a public domain without credentials.',
      })
    }

    return hostname
  } catch {
    throw new TargetedSiteOutreachValidationError({
      reason: 'targetDomain must be a public domain without credentials.',
    })
  }
}

const normalizeOriginUrl = (
  originUrl: string | undefined,
  normalizedDomain: string,
): string | null => {
  if (originUrl === undefined) {
    return `https://${normalizedDomain}/`
  }

  if (!textIsSafe(originUrl)) {
    throw new TargetedSiteOutreachValidationError({
      reason:
        'originUrl must not contain raw contact, provider, payment, wallet, or operator-note material.',
    })
  }

  try {
    const url = new URL(originUrl)

    if (url.protocol !== 'https:' && url.protocol !== 'http:') {
      throw new TargetedSiteOutreachValidationError({
        reason: 'originUrl must be an http(s) URL for the normalized domain.',
      })
    }

    if (url.username !== '' || url.password !== '') {
      throw new TargetedSiteOutreachValidationError({
        reason: 'originUrl must be an http(s) URL without credentials.',
      })
    }

    const hostname = url.hostname.replace(/^www\./, '').toLowerCase()

    if (hostname !== normalizedDomain) {
      throw new TargetedSiteOutreachValidationError({
        reason: 'originUrl must be an http(s) URL for the normalized domain.',
      })
    }

    url.hash = ''

    return url.toString()
  } catch {
    throw new TargetedSiteOutreachValidationError({
      reason: 'originUrl must be an http(s) URL for the normalized domain.',
    })
  }
}

const normalizedConfidence = (value: number): number => {
  if (!Number.isFinite(value)) {
    throw new TargetedSiteOutreachValidationError({
      reason: 'discoveryConfidence must be finite.',
    })
  }

  return Math.max(0, Math.min(1, value))
}

const assertCampaignInput = (
  input: CreateTargetedSiteCampaignInput,
): void => {
  if (!SAFE_SLUG_PATTERN.test(input.slug) || !textIsSafe(input.slug)) {
    throw new TargetedSiteOutreachValidationError({
      reason: 'slug must be a public-safe campaign slug.',
    })
  }

  assertSafeText('name', input.name)
  assertSafeText('vertical', input.vertical)
  assertSafeText('geography', input.geography)
  assertSafeRef('id', input.id)
  assertSafeRef('ownerUserId', input.ownerUserId)
  assertSafeRef('operatorUserId', input.operatorUserId)
  assertSafeRef('sourceAuthorityRef', input.sourceAuthorityRef)
  assertSafeRef('budgetCapRef', input.budgetCapRef)
  assertSafeRef('suppressionPolicyRef', input.suppressionPolicyRef)
  assertSafeMetadata(input.metadata)
}

const assertProspectInput = (
  input: UpsertTargetedSiteProspectInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('campaignId', input.campaignId)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('sourceRef', input.sourceRef)
  input.contactRefs?.forEach(contactRef =>
    assertSafeRef('contactRefs', contactRef),
  )
  assertSafeText('companyName', input.companyName)
  assertSafeText('siteName', input.siteName)
  assertSafeText('vertical', input.vertical)
  assertSafeText('geography', input.geography)
  assertSafeMetadata(input.metadata)
}

const metadataFromJson = (json: string): Readonly<Record<string, unknown>> =>
  parseJsonRecord(json) ?? {}

const campaignFromRow = (
  row: TargetedSiteCampaignRow,
): TargetedSiteCampaignRecord => ({
  archivedAt: row.archived_at,
  budgetCapRef: row.budget_cap_ref,
  createdAt: row.created_at,
  geography: row.geography,
  id: row.id,
  metadata: metadataFromJson(row.metadata_json),
  name: row.name,
  operatorState: row.operator_state,
  operatorUserId: row.operator_user_id,
  ownerUserId: row.owner_user_id,
  slug: row.slug,
  sourceAuthorityRef: row.source_authority_ref,
  suppressionPolicyRef: row.suppression_policy_ref,
  updatedAt: row.updated_at,
  vertical: row.vertical,
})

const prospectFromRow = (
  row: TargetedSiteProspectRow,
): TargetedSiteProspectRecord => ({
  archivedAt: row.archived_at,
  campaignId: row.campaign_id,
  captureState: row.capture_state,
  companyName: row.company_name,
  contactRefs: [...parseJsonStringArray(row.contact_refs_json)],
  createdAt: row.created_at,
  discoveredAt: row.discovered_at,
  discoveryConfidence: Number(row.discovery_confidence),
  geography: row.geography,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: metadataFromJson(row.metadata_json),
  normalizedDomain: row.normalized_domain,
  originUrl: row.origin_url,
  reviewState: row.review_state,
  siteName: row.site_name,
  sourceRef: row.source_ref,
  suppressionState: row.suppression_state,
  updatedAt: row.updated_at,
  vertical: row.vertical,
})

export class TargetedSiteOutreachValidationError extends S.TaggedErrorClass<TargetedSiteOutreachValidationError>()(
  'TargetedSiteOutreachValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteOutreachStorageError extends S.TaggedErrorClass<TargetedSiteOutreachStorageError>()(
  'TargetedSiteOutreachStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export const createTargetedSiteCampaign = async (
  db: D1Database,
  input: CreateTargetedSiteCampaignInput,
): Promise<TargetedSiteCampaignRecord> => {
  assertCampaignInput(input)

  const nowIso = currentIsoTimestamp()
  const id = input.id ?? compactRandomId('targeted_site_campaign')
  const name = compactText(input.name, 160)

  await db
    .prepare(
      `INSERT OR IGNORE INTO targeted_site_campaigns (
         id,
         slug,
         name,
         owner_user_id,
         operator_user_id,
         vertical,
         geography,
         source_authority_ref,
         budget_cap_ref,
         suppression_policy_ref,
         operator_state,
         metadata_json,
         created_at,
         updated_at,
         archived_at
       ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
    )
    .bind(
      id,
      input.slug,
      name,
      input.ownerUserId,
      input.operatorUserId ?? null,
      optionalText(input.vertical, 120) ?? null,
      optionalText(input.geography, 120) ?? null,
      input.sourceAuthorityRef,
      input.budgetCapRef ?? null,
      input.suppressionPolicyRef ?? null,
      input.operatorState ?? 'draft',
      JSON.stringify(input.metadata ?? {}),
      nowIso,
      nowIso,
    )
    .run()

  const campaign = await readTargetedSiteCampaignBySlug(db, input.slug)

  if (campaign === null) {
    throw new TargetedSiteOutreachStorageError({
      operation: 'createTargetedSiteCampaign.readBySlug',
      reason: 'inserted or existing targeted Site campaign was not readable.',
    })
  }

  return campaign
}

export const readTargetedSiteCampaignBySlug = async (
  db: D1Database,
  slug: string,
): Promise<TargetedSiteCampaignRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM targeted_site_campaigns
        WHERE slug = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(slug)
    .first<TargetedSiteCampaignRow>()

  return row === null ? null : campaignFromRow(row)
}

export const listTargetedSiteCampaignsByOwner = async (
  db: D1Database,
  ownerUserId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteCampaignRecord>> => {
  assertSafeRef('ownerUserId', ownerUserId)

  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM targeted_site_campaigns
        WHERE owner_user_id = ?
          AND archived_at IS NULL
        ORDER BY updated_at DESC
        LIMIT ?`,
    )
    .bind(ownerUserId, safeLimit)
    .all<TargetedSiteCampaignRow>()

  return (rows.results ?? []).map(campaignFromRow)
}

export const listTargetedSiteCampaignsByOperator = async (
  db: D1Database,
  operatorUserId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteCampaignRecord>> => {
  assertSafeRef('operatorUserId', operatorUserId)

  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM targeted_site_campaigns
        WHERE operator_user_id = ?
          AND archived_at IS NULL
        ORDER BY updated_at DESC
        LIMIT ?`,
    )
    .bind(operatorUserId, safeLimit)
    .all<TargetedSiteCampaignRow>()

  return (rows.results ?? []).map(campaignFromRow)
}

const readProspectByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<TargetedSiteProspectRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM targeted_site_prospects
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<TargetedSiteProspectRow>()

  return row === null ? null : prospectFromRow(row)
}

const readProspectByCampaignDomain = async (
  db: D1Database,
  campaignId: string,
  normalizedDomain: string,
): Promise<TargetedSiteProspectRecord | null> => {
  const row = await db
    .prepare(
      `SELECT *
         FROM targeted_site_prospects
        WHERE campaign_id = ?
          AND normalized_domain = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(campaignId, normalizedDomain)
    .first<TargetedSiteProspectRow>()

  return row === null ? null : prospectFromRow(row)
}

export const upsertTargetedSiteProspect = async (
  db: D1Database,
  input: UpsertTargetedSiteProspectInput,
): Promise<TargetedSiteProspectRecord> => {
  assertProspectInput(input)

  const nowIso = currentIsoTimestamp()
  const normalizedDomain = normalizeDomain(input.targetDomain)
  const originUrl = normalizeOriginUrl(input.originUrl, normalizedDomain)
  const existingByIdempotency = await readProspectByIdempotencyKey(
    db,
    input.idempotencyKey,
  )

  if (existingByIdempotency !== null) {
    return existingByIdempotency
  }

  const existingByDomain = await readProspectByCampaignDomain(
    db,
    input.campaignId,
    normalizedDomain,
  )
  const id = input.id ?? compactRandomId('targeted_site_prospect')
  const contactRefsJson = JSON.stringify([...(input.contactRefs ?? [])])
  const metadataJson = JSON.stringify(input.metadata ?? {})
  const values = [
    optionalText(input.companyName, 160) ?? null,
    optionalText(input.siteName, 160) ?? null,
    contactRefsJson,
    optionalText(input.vertical, 120) ?? null,
    optionalText(input.geography, 120) ?? null,
    input.sourceRef,
    normalizedConfidence(input.discoveryConfidence),
    input.suppressionState ?? 'unknown',
    input.captureState ?? 'not_started',
    input.reviewState ?? 'pending',
    metadataJson,
    nowIso,
  ] as const

  if (existingByDomain === null) {
    await db
      .prepare(
        `INSERT INTO targeted_site_prospects (
           id,
           campaign_id,
           idempotency_key,
           normalized_domain,
           origin_url,
           company_name,
           site_name,
           contact_refs_json,
           vertical,
           geography,
           source_ref,
           discovery_confidence,
           suppression_state,
           capture_state,
           review_state,
           metadata_json,
           discovered_at,
           created_at,
           updated_at,
           archived_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        id,
        input.campaignId,
        input.idempotencyKey,
        normalizedDomain,
        originUrl,
        ...values,
        nowIso,
        nowIso,
      )
      .run()
  } else {
    await db
      .prepare(
        `UPDATE targeted_site_prospects
            SET origin_url = ?,
                company_name = ?,
                site_name = ?,
                contact_refs_json = ?,
                vertical = ?,
                geography = ?,
                source_ref = ?,
                discovery_confidence = ?,
                suppression_state = ?,
                capture_state = ?,
                review_state = ?,
                metadata_json = ?,
                updated_at = ?
          WHERE campaign_id = ?
            AND normalized_domain = ?
            AND archived_at IS NULL`,
      )
      .bind(
        originUrl,
        ...values,
        input.campaignId,
        normalizedDomain,
      )
      .run()
  }

  const prospect = await readProspectByCampaignDomain(
    db,
    input.campaignId,
    normalizedDomain,
  )

  if (prospect === null) {
    throw new TargetedSiteOutreachStorageError({
      operation: 'upsertTargetedSiteProspect.readByCampaignDomain',
      reason: 'inserted or updated targeted Site prospect was not readable.',
    })
  }

  return prospect
}

export const listTargetedSiteProspectsByCampaign = async (
  db: D1Database,
  campaignId: string,
  limit = 100,
): Promise<ReadonlyArray<TargetedSiteProspectRecord>> => {
  assertSafeRef('campaignId', campaignId)

  const safeLimit = Math.max(1, Math.min(200, Math.floor(limit)))
  const rows = await db
    .prepare(
      `SELECT *
         FROM targeted_site_prospects
        WHERE campaign_id = ?
          AND archived_at IS NULL
        ORDER BY updated_at DESC
        LIMIT ?`,
    )
    .bind(campaignId, safeLimit)
    .all<TargetedSiteProspectRow>()

  return (rows.results ?? []).map(prospectFromRow)
}
