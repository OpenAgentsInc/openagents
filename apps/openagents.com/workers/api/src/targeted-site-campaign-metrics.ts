import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const TargetedSiteCampaignMetricEventKind = S.Literals([
  'capture_cost',
  'preview_generated',
  'outreach_sent',
  'email_bounced',
  'email_replied',
  'meeting_booked',
  'customer_converted',
  'accepted_outcome',
  'refund',
  'complaint',
  'suppressed',
  'blocked',
])
export type TargetedSiteCampaignMetricEventKind =
  typeof TargetedSiteCampaignMetricEventKind.Type

export const TargetedSiteCampaignMetricEventRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  campaignId: S.String,
  costCents: S.Number,
  createdAt: S.String,
  eventKind: TargetedSiteCampaignMetricEventKind,
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  normalizedDomain: S.NullOr(S.String),
  occurredAt: S.String,
  prospectId: S.NullOr(S.String),
  publicRef: S.NullOr(S.String),
  quantity: S.Number,
  relatedEventId: S.NullOr(S.String),
  sourceRef: S.String,
})
export type TargetedSiteCampaignMetricEventRecord =
  typeof TargetedSiteCampaignMetricEventRecord.Type

export const TargetedSiteCampaignMetricsProjection = S.Struct({
  acceptedOutcomeCount: S.Number,
  blockedCount: S.Number,
  bounceCount: S.Number,
  campaignId: S.String,
  complaintCount: S.Number,
  conversionCount: S.Number,
  eventCount: S.Number,
  latestEventAt: S.NullOr(S.String),
  meetingCount: S.Number,
  previewCount: S.Number,
  refundCount: S.Number,
  replyCount: S.Number,
  sentCount: S.Number,
  suppressedCount: S.Number,
  totalCaptureCostCents: S.Number,
})
export type TargetedSiteCampaignMetricsProjection =
  typeof TargetedSiteCampaignMetricsProjection.Type

export type TargetedSiteCampaignMetricsRuntime = Readonly<{
  makeEventId: () => string
  nowIso: () => string
}>

export const systemTargetedSiteCampaignMetricsRuntime: TargetedSiteCampaignMetricsRuntime =
  {
    makeEventId: () => compactRandomId('targeted_site_campaign_metric'),
    nowIso: currentIsoTimestamp,
  }

export type RecordTargetedSiteCampaignMetricEventInput = Readonly<{
  campaignId: string
  costCents?: number | undefined
  eventKind: TargetedSiteCampaignMetricEventKind
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  normalizedDomain?: string | undefined
  occurredAt?: string | undefined
  prospectId?: string | undefined
  publicRef?: string | undefined
  quantity?: number | undefined
  relatedEventId?: string | undefined
  sourceRef: string
}>

type TargetedSiteCampaignMetricEventRow = Readonly<{
  archived_at: string | null
  campaign_id: string
  cost_cents: number
  created_at: string
  event_kind: TargetedSiteCampaignMetricEventKind
  id: string
  idempotency_key: string
  metadata_json: string
  normalized_domain: string | null
  occurred_at: string
  prospect_id: string | null
  public_ref: string | null
  quantity: number
  related_event_id: string | null
  source_ref: string
}>

type MetricAggregateRow = Readonly<{
  accepted_outcome_count: number
  blocked_count: number
  bounce_count: number
  campaign_id: string
  complaint_count: number
  conversion_count: number
  event_count: number
  latest_event_at: string | null
  meeting_count: number
  preview_count: number
  refund_count: number
  reply_count: number
  sent_count: number
  suppressed_count: number
  total_capture_cost_cents: number
}>

export class TargetedSiteCampaignMetricValidationError extends S.TaggedErrorClass<TargetedSiteCampaignMetricValidationError>()(
  'TargetedSiteCampaignMetricValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteCampaignMetricStorageError extends S.TaggedErrorClass<TargetedSiteCampaignMetricStorageError>()(
  'TargetedSiteCampaignMetricStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class TargetedSiteCampaignMetricCampaignNotFound extends S.TaggedErrorClass<TargetedSiteCampaignMetricCampaignNotFound>()(
  'TargetedSiteCampaignMetricCampaignNotFound',
  {
    campaignId: S.String,
  },
) {}

export class TargetedSiteCampaignMetricProspectNotFound extends S.TaggedErrorClass<TargetedSiteCampaignMetricProspectNotFound>()(
  'TargetedSiteCampaignMetricProspectNotFound',
  {
    prospectId: S.String,
  },
) {}

export type TargetedSiteCampaignMetricError =
  | TargetedSiteCampaignMetricCampaignNotFound
  | TargetedSiteCampaignMetricProspectNotFound
  | TargetedSiteCampaignMetricStorageError
  | TargetedSiteCampaignMetricValidationError

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_DOMAIN_PATTERN =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{1,62}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?email|email[_ -]?body|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic)\b|@/i

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteCampaignMetricValidationError({
      reason: `${field} must be a public-safe ref without raw provider, email, payment, wallet, or private customer material.`,
    })
  }
}

const assertSafeDomain = (value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_DOMAIN_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteCampaignMetricValidationError({
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
    throw new TargetedSiteCampaignMetricValidationError({
      reason:
        'metadata must not contain raw provider, email, payment, wallet, or private customer material.',
    })
  }
}

const assertValidInput = (
  input: RecordTargetedSiteCampaignMetricEventInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('campaignId', input.campaignId)
  assertSafeRef('prospectId', input.prospectId)
  assertSafeRef('publicRef', input.publicRef)
  assertSafeRef('relatedEventId', input.relatedEventId)
  assertSafeRef('sourceRef', input.sourceRef)
  assertSafeDomain(input.normalizedDomain)
  assertSafeMetadata(input.metadata)

  if (input.quantity !== undefined && (!Number.isSafeInteger(input.quantity) || input.quantity < 0)) {
    throw new TargetedSiteCampaignMetricValidationError({
      reason: 'quantity must be a non-negative integer.',
    })
  }

  if (input.costCents !== undefined && (!Number.isSafeInteger(input.costCents) || input.costCents < 0)) {
    throw new TargetedSiteCampaignMetricValidationError({
      reason: 'costCents must be a non-negative integer.',
    })
  }

  if (
    (input.eventKind === 'refund' || input.eventKind === 'complaint') &&
    input.relatedEventId === undefined
  ) {
    throw new TargetedSiteCampaignMetricValidationError({
      reason: 'refund and complaint metric events must link to a related event.',
    })
  }
}

const storageError = (
  operation: string,
  error: unknown,
): TargetedSiteCampaignMetricStorageError =>
  new TargetedSiteCampaignMetricStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, TargetedSiteCampaignMetricStorageError> =>
  Effect.tryPromise({
    catch: error => storageError(operation, error),
    try: run,
  })

const metricEventFromRow = (
  row: TargetedSiteCampaignMetricEventRow,
): TargetedSiteCampaignMetricEventRecord => ({
  archivedAt: row.archived_at,
  campaignId: row.campaign_id,
  costCents: row.cost_cents,
  createdAt: row.created_at,
  eventKind: row.event_kind,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  normalizedDomain: row.normalized_domain,
  occurredAt: row.occurred_at,
  prospectId: row.prospect_id,
  publicRef: row.public_ref,
  quantity: row.quantity,
  relatedEventId: row.related_event_id,
  sourceRef: row.source_ref,
})

const readByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  TargetedSiteCampaignMetricEventRecord | null,
  TargetedSiteCampaignMetricStorageError
> =>
  d1Effect('targetedSiteCampaignMetrics.readByIdempotencyKey', () =>
    db
      .prepare(
        `SELECT *
           FROM targeted_site_campaign_metric_events
          WHERE idempotency_key = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(idempotencyKey)
      .first<TargetedSiteCampaignMetricEventRow>(),
  ).pipe(Effect.map(row => (row === null ? null : metricEventFromRow(row))))

const activeCampaignExists = (
  db: D1Database,
  campaignId: string,
): Effect.Effect<boolean, TargetedSiteCampaignMetricStorageError> =>
  d1Effect('targetedSiteCampaignMetrics.campaign.exists', () =>
    db
      .prepare(
        `SELECT id
           FROM targeted_site_campaigns
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(campaignId)
      .first<{ id: string }>(),
  ).pipe(Effect.map(row => row !== null))

const activeProspectExists = (
  db: D1Database,
  campaignId: string,
  prospectId: string,
): Effect.Effect<boolean, TargetedSiteCampaignMetricStorageError> =>
  d1Effect('targetedSiteCampaignMetrics.prospect.exists', () =>
    db
      .prepare(
        `SELECT id
           FROM targeted_site_prospects
          WHERE id = ?
            AND campaign_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(prospectId, campaignId)
      .first<{ id: string }>(),
  ).pipe(Effect.map(row => row !== null))

export const recordTargetedSiteCampaignMetricEvent = (
  db: D1Database,
  input: RecordTargetedSiteCampaignMetricEventInput,
  runtime: TargetedSiteCampaignMetricsRuntime =
    systemTargetedSiteCampaignMetricsRuntime,
): Effect.Effect<
  TargetedSiteCampaignMetricEventRecord,
  TargetedSiteCampaignMetricError
> =>
  Effect.gen(function* () {
    assertValidInput(input)
    const existing = yield* readByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    const campaignExists = yield* activeCampaignExists(db, input.campaignId)

    if (!campaignExists) {
      return yield* new TargetedSiteCampaignMetricCampaignNotFound({
        campaignId: input.campaignId,
      })
    }

    if (input.prospectId !== undefined) {
      const prospectExists = yield* activeProspectExists(
        db,
        input.campaignId,
        input.prospectId,
      )

      if (!prospectExists) {
        return yield* new TargetedSiteCampaignMetricProspectNotFound({
          prospectId: input.prospectId,
        })
      }
    }

    const now = runtime.nowIso()
    const record: TargetedSiteCampaignMetricEventRecord = {
      archivedAt: null,
      campaignId: input.campaignId,
      costCents: input.costCents ?? 0,
      createdAt: now,
      eventKind: input.eventKind,
      id: input.id ?? runtime.makeEventId(),
      idempotencyKey: input.idempotencyKey,
      metadata: input.metadata ?? {},
      normalizedDomain: input.normalizedDomain ?? null,
      occurredAt: input.occurredAt ?? now,
      prospectId: input.prospectId ?? null,
      publicRef: input.publicRef ?? null,
      quantity: input.quantity ?? 1,
      relatedEventId: input.relatedEventId ?? null,
      sourceRef: input.sourceRef,
    }

    yield* d1Effect('targetedSiteCampaignMetrics.insert', () =>
      db
        .prepare(
          `INSERT OR IGNORE INTO targeted_site_campaign_metric_events
             (id,
              idempotency_key,
              campaign_id,
              prospect_id,
              normalized_domain,
              event_kind,
              quantity,
              cost_cents,
              public_ref,
              source_ref,
              related_event_id,
              metadata_json,
              occurred_at,
              created_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          record.id,
          record.idempotencyKey,
          record.campaignId,
          record.prospectId,
          record.normalizedDomain,
          record.eventKind,
          record.quantity,
          record.costCents,
          record.publicRef,
          record.sourceRef,
          record.relatedEventId,
          JSON.stringify(record.metadata),
          record.occurredAt,
          record.createdAt,
        )
        .run()
        .then(() => undefined),
    )

    return (yield* readByIdempotencyKey(db, record.idempotencyKey)) ?? record
  })

const emptyProjection = (
  campaignId: string,
): TargetedSiteCampaignMetricsProjection => ({
  acceptedOutcomeCount: 0,
  blockedCount: 0,
  bounceCount: 0,
  campaignId,
  complaintCount: 0,
  conversionCount: 0,
  eventCount: 0,
  latestEventAt: null,
  meetingCount: 0,
  previewCount: 0,
  refundCount: 0,
  replyCount: 0,
  sentCount: 0,
  suppressedCount: 0,
  totalCaptureCostCents: 0,
})

const projectionFromAggregateRow = (
  row: MetricAggregateRow,
): TargetedSiteCampaignMetricsProjection => ({
  acceptedOutcomeCount: row.accepted_outcome_count,
  blockedCount: row.blocked_count,
  bounceCount: row.bounce_count,
  campaignId: row.campaign_id,
  complaintCount: row.complaint_count,
  conversionCount: row.conversion_count,
  eventCount: row.event_count,
  latestEventAt: row.latest_event_at,
  meetingCount: row.meeting_count,
  previewCount: row.preview_count,
  refundCount: row.refund_count,
  replyCount: row.reply_count,
  sentCount: row.sent_count,
  suppressedCount: row.suppressed_count,
  totalCaptureCostCents: row.total_capture_cost_cents,
})

export const projectTargetedSiteCampaignMetrics = (
  db: D1Database,
  campaignId: string,
): Effect.Effect<
  TargetedSiteCampaignMetricsProjection,
  TargetedSiteCampaignMetricError
> =>
  Effect.gen(function* () {
    assertSafeRef('campaignId', campaignId)
    const campaignExists = yield* activeCampaignExists(db, campaignId)

    if (!campaignExists) {
      return yield* new TargetedSiteCampaignMetricCampaignNotFound({ campaignId })
    }

    const row = yield* d1Effect('targetedSiteCampaignMetrics.project', () =>
      db
        .prepare(
          `SELECT ? AS campaign_id,
                  COUNT(*) AS event_count,
                  MAX(occurred_at) AS latest_event_at,
                  COALESCE(SUM(CASE WHEN event_kind = 'capture_cost' THEN cost_cents ELSE 0 END), 0) AS total_capture_cost_cents,
                  COALESCE(SUM(CASE WHEN event_kind = 'preview_generated' THEN quantity ELSE 0 END), 0) AS preview_count,
                  COALESCE(SUM(CASE WHEN event_kind = 'outreach_sent' THEN quantity ELSE 0 END), 0) AS sent_count,
                  COALESCE(SUM(CASE WHEN event_kind = 'email_bounced' THEN quantity ELSE 0 END), 0) AS bounce_count,
                  COALESCE(SUM(CASE WHEN event_kind = 'email_replied' THEN quantity ELSE 0 END), 0) AS reply_count,
                  COALESCE(SUM(CASE WHEN event_kind = 'meeting_booked' THEN quantity ELSE 0 END), 0) AS meeting_count,
                  COALESCE(SUM(CASE WHEN event_kind = 'customer_converted' THEN quantity ELSE 0 END), 0) AS conversion_count,
                  COALESCE(SUM(CASE WHEN event_kind = 'accepted_outcome' THEN quantity ELSE 0 END), 0) AS accepted_outcome_count,
                  COALESCE(SUM(CASE WHEN event_kind = 'refund' THEN quantity ELSE 0 END), 0) AS refund_count,
                  COALESCE(SUM(CASE WHEN event_kind = 'complaint' THEN quantity ELSE 0 END), 0) AS complaint_count,
                  COALESCE(SUM(CASE WHEN event_kind = 'suppressed' THEN quantity ELSE 0 END), 0) AS suppressed_count,
                  COALESCE(SUM(CASE WHEN event_kind = 'blocked' THEN quantity ELSE 0 END), 0) AS blocked_count
             FROM targeted_site_campaign_metric_events
            WHERE campaign_id = ?
              AND archived_at IS NULL`,
        )
        .bind(campaignId, campaignId)
        .first<MetricAggregateRow>(),
    )

    return row === null ? emptyProjection(campaignId) : projectionFromAggregateRow(row)
  })

export const publicTargetedSiteCampaignMetricsProjection = (
  projection: TargetedSiteCampaignMetricsProjection,
) => ({
  acceptedOutcomeCount: projection.acceptedOutcomeCount,
  blockedCount: projection.blockedCount,
  bounceCount: projection.bounceCount,
  campaignId: projection.campaignId,
  complaintCount: projection.complaintCount,
  conversionCount: projection.conversionCount,
  latestEventAt: projection.latestEventAt,
  meetingCount: projection.meetingCount,
  previewCount: projection.previewCount,
  refundCount: projection.refundCount,
  replyCount: projection.replyCount,
  sentCount: projection.sentCount,
  suppressedCount: projection.suppressedCount,
  totalCaptureCostCents: projection.totalCaptureCostCents,
})
