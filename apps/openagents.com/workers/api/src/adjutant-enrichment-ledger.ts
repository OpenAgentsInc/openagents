import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

// KS-8.11 (#8322): enrichment ledger functions take the `CrmEmailDatabase`
// union — a plain D1Database keeps working (no mirroring); the dual-write
// handle converges the Postgres twins fail-soft after each D1 write.
import {
  type CrmEmailDatabase,
  crmEmailAuthorityDb,
  mirrorCrmEmailRows,
} from './crm-email-domain-store'
import { openAgentsDatabase } from './runtime'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

type AdjutantEnrichmentLedgerEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

export type AdjutantEnrichmentLedgerRuntime = Readonly<{
  makeQueryId: () => string
  makeRunId: () => string
  makeSourceId: () => string
  nowIso: () => string
}>

export const systemAdjutantEnrichmentLedgerRuntime: AdjutantEnrichmentLedgerRuntime =
  {
    makeQueryId: () => compactRandomId('exa_enrichment_query'),
    makeRunId: () => compactRandomId('exa_enrichment_run'),
    makeSourceId: () => compactRandomId('exa_enrichment_source'),
    nowIso: currentIsoTimestamp,
  }

const MAX_QUERY_TEXT_CHARS = 500
const MAX_SUBJECT_CHARS = 500
const MAX_SOURCE_CARD_JSON_BYTES = 4096
const MAX_SOURCE_DOMAIN_CHARS = 255
const MAX_SOURCE_HIGHLIGHT_CHARS = 1200
const MAX_SOURCE_TITLE_CHARS = 240
const MAX_SOURCE_URL_CHARS = 2048
const MAX_ERROR_SUMMARY_CHARS = 500

export const ExaEnrichmentRunStatus = S.Literals([
  'planned',
  'queued',
  'running',
  'succeeded',
  'partial_failure',
  'failed',
  'needs_review',
  'approved',
  'rejected',
  'stale',
])
export type ExaEnrichmentRunStatus = typeof ExaEnrichmentRunStatus.Type

export const ExaEnrichmentQueryStatus = S.Literals([
  'planned',
  'running',
  'succeeded',
  'failed',
  'cached',
])
export type ExaEnrichmentQueryStatus = typeof ExaEnrichmentQueryStatus.Type

export const ExaSourceReviewStatus = S.Literals([
  'proposed',
  'approved',
  'rejected',
  'internal_only',
  'public_safe',
])
export type ExaSourceReviewStatus = typeof ExaSourceReviewStatus.Type

export const ExaSourceCategory = S.Literals([
  'order_request',
  'topic_web',
  'repository',
  'github_profile',
  'personal_site',
  'linkedin_profile',
  'x_profile',
  'people_profile',
  'generic_url',
])
export type ExaSourceCategory = typeof ExaSourceCategory.Type

export const ExaEnrichmentRun = S.Struct({
  id: S.String,
  assignmentId: S.String,
  softwareOrderId: S.NullOr(S.String),
  siteId: S.NullOr(S.String),
  planId: S.String,
  subject: S.String,
  status: ExaEnrichmentRunStatus,
  requestBudget: S.Number,
  requestCount: S.Number,
  cacheHitCount: S.Number,
  sourceCount: S.Number,
  approvedSourceCount: S.Number,
  costDollars: S.NullOr(S.Number),
  errorCode: S.NullOr(S.String),
  errorSummary: S.NullOr(S.String),
  startedAt: S.NullOr(S.String),
  completedAt: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
  archivedAt: S.NullOr(S.String),
})
export type ExaEnrichmentRun = typeof ExaEnrichmentRun.Type

export const ExaEnrichmentQuery = S.Struct({
  id: S.String,
  runId: S.String,
  assignmentId: S.String,
  queryHash: S.String,
  queryText: S.String,
  sourceCategory: ExaSourceCategory,
  searchType: S.String,
  freshnessMaxAgeHours: S.Number,
  status: ExaEnrichmentQueryStatus,
  resultCount: S.Number,
  latencyMs: S.NullOr(S.Number),
  costDollars: S.NullOr(S.Number),
  errorCode: S.NullOr(S.String),
  errorSummary: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
})
export type ExaEnrichmentQuery = typeof ExaEnrichmentQuery.Type

export const ExaEnrichmentSourceCard = S.Struct({
  id: S.String,
  runId: S.String,
  queryId: S.NullOr(S.String),
  assignmentId: S.String,
  softwareOrderId: S.NullOr(S.String),
  siteId: S.NullOr(S.String),
  sourceCategory: ExaSourceCategory,
  reviewStatus: ExaSourceReviewStatus,
  title: S.String,
  url: S.String,
  domain: S.String,
  publishedDate: S.NullOr(S.String),
  highlightText: S.NullOr(S.String),
  selectedTextHash: S.NullOr(S.String),
  exaRequestId: S.NullOr(S.String),
  searchType: S.NullOr(S.String),
  publicSafe: S.Boolean,
  rejectedReason: S.NullOr(S.String),
  approvedAt: S.NullOr(S.String),
  rejectedAt: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
})
export type ExaEnrichmentSourceCard =
  typeof ExaEnrichmentSourceCard.Type

export const CreateExaEnrichmentRunInput = S.Struct({
  assignmentId: S.String,
  planId: S.String,
  requestBudget: S.optionalKey(S.Number),
  siteId: S.optionalKey(S.NullOr(S.String)),
  softwareOrderId: S.optionalKey(S.NullOr(S.String)),
  startedAt: S.optionalKey(S.NullOr(S.String)),
  status: S.optionalKey(ExaEnrichmentRunStatus),
  subject: S.String,
})
export type CreateExaEnrichmentRunInput =
  typeof CreateExaEnrichmentRunInput.Type

export const RecordExaEnrichmentQueryInput = S.Struct({
  assignmentId: S.String,
  costDollars: S.optionalKey(S.Number),
  errorCode: S.optionalKey(S.String),
  errorSummary: S.optionalKey(S.String),
  freshnessMaxAgeHours: S.Number,
  latencyMs: S.optionalKey(S.Number),
  queryText: S.String,
  resultCount: S.optionalKey(S.Number),
  runId: S.String,
  searchType: S.String,
  sourceCategory: ExaSourceCategory,
  status: S.optionalKey(ExaEnrichmentQueryStatus),
})
export type RecordExaEnrichmentQueryInput =
  typeof RecordExaEnrichmentQueryInput.Type

export const StoreExaSourceCardInput = S.Struct({
  assignmentId: S.String,
  domain: S.optionalKey(S.String),
  exaRequestId: S.optionalKey(S.NullOr(S.String)),
  highlightText: S.optionalKey(S.NullOr(S.String)),
  publishedDate: S.optionalKey(S.NullOr(S.String)),
  queryId: S.optionalKey(S.NullOr(S.String)),
  rejectedReason: S.optionalKey(S.NullOr(S.String)),
  reviewStatus: S.optionalKey(ExaSourceReviewStatus),
  runId: S.String,
  searchType: S.optionalKey(S.NullOr(S.String)),
  selectedText: S.optionalKey(S.NullOr(S.String)),
  siteId: S.optionalKey(S.NullOr(S.String)),
  softwareOrderId: S.optionalKey(S.NullOr(S.String)),
  sourceCategory: ExaSourceCategory,
  title: S.String,
  url: S.String,
})
export type StoreExaSourceCardInput = typeof StoreExaSourceCardInput.Type

export const ReviewExaSourceCardInput = S.Struct({
  publicSafe: S.optionalKey(S.Boolean),
  rejectedReason: S.optionalKey(S.NullOr(S.String)),
  reviewStatus: ExaSourceReviewStatus,
  sourceId: S.String,
})
export type ReviewExaSourceCardInput = typeof ReviewExaSourceCardInput.Type

export const LinkAssignmentEnrichmentInput = S.Struct({
  assignmentId: S.String,
  enrichmentRunId: S.String,
  requiredForLaunch: S.optionalKey(S.Boolean),
  researchBriefId: S.optionalKey(S.NullOr(S.String)),
  status: S.Literals([
    'planned',
    'running',
    'needs_review',
    'approved',
    'rejected',
    'stale',
    'failed',
  ]),
})
export type LinkAssignmentEnrichmentInput =
  typeof LinkAssignmentEnrichmentInput.Type

export const UpdateExaEnrichmentRunStatusInput = S.Struct({
  completedAt: S.optionalKey(S.NullOr(S.String)),
  errorCode: S.optionalKey(S.NullOr(S.String)),
  errorSummary: S.optionalKey(S.NullOr(S.String)),
  runId: S.String,
  status: ExaEnrichmentRunStatus,
})
export type UpdateExaEnrichmentRunStatusInput =
  typeof UpdateExaEnrichmentRunStatusInput.Type

type ExaEnrichmentRunRow = Readonly<{
  approved_source_count: number
  archived_at: string | null
  assignment_id: string
  cache_hit_count: number
  completed_at: string | null
  cost_dollars: number | null
  created_at: string
  error_code: string | null
  error_summary: string | null
  id: string
  plan_id: string
  request_budget: number
  request_count: number
  site_id: string | null
  software_order_id: string | null
  source_count: number
  started_at: string | null
  status: ExaEnrichmentRunStatus
  subject: string
  updated_at: string
}>

type ExaEnrichmentQueryRow = Readonly<{
  assignment_id: string
  cost_dollars: number | null
  created_at: string
  error_code: string | null
  error_summary: string | null
  freshness_max_age_hours: number
  id: string
  latency_ms: number | null
  query_hash: string
  query_text: string
  result_count: number
  run_id: string
  search_type: string
  source_category: ExaSourceCategory
  status: ExaEnrichmentQueryStatus
  updated_at: string
}>

type ExaEnrichmentSourceRow = Readonly<{
  approved_at: string | null
  assignment_id: string
  created_at: string
  domain: string
  exa_request_id: string | null
  highlight_text: string | null
  id: string
  public_safe: number
  published_date: string | null
  query_id: string | null
  rejected_at: string | null
  rejected_reason: string | null
  review_status: ExaSourceReviewStatus
  run_id: string
  search_type: string | null
  selected_text_hash: string | null
  site_id: string | null
  software_order_id: string | null
  source_category: ExaSourceCategory
  title: string
  updated_at: string
  url: string
}>

export class AdjutantEnrichmentLedgerStorageError extends S.TaggedErrorClass<AdjutantEnrichmentLedgerStorageError>()(
  'AdjutantEnrichmentLedgerStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class AdjutantEnrichmentLedgerUnsafePayload extends S.TaggedErrorClass<AdjutantEnrichmentLedgerUnsafePayload>()(
  'AdjutantEnrichmentLedgerUnsafePayload',
  {
    reason: S.String,
  },
) {}

export class AdjutantEnrichmentLedgerValidationError extends S.TaggedErrorClass<AdjutantEnrichmentLedgerValidationError>()(
  'AdjutantEnrichmentLedgerValidationError',
  {
    reason: S.String,
  },
) {}

export type AdjutantEnrichmentLedgerError =
  | AdjutantEnrichmentLedgerStorageError
  | AdjutantEnrichmentLedgerUnsafePayload
  | AdjutantEnrichmentLedgerValidationError

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AdjutantEnrichmentLedgerStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error =>
      new AdjutantEnrichmentLedgerStorageError({ operation, error }),
  })

const runFromRow = (row: ExaEnrichmentRunRow): ExaEnrichmentRun => ({
  id: row.id,
  assignmentId: row.assignment_id,
  softwareOrderId: row.software_order_id,
  siteId: row.site_id,
  planId: row.plan_id,
  subject: row.subject,
  status: row.status,
  requestBudget: row.request_budget,
  requestCount: row.request_count,
  cacheHitCount: row.cache_hit_count,
  sourceCount: row.source_count,
  approvedSourceCount: row.approved_source_count,
  costDollars: row.cost_dollars,
  errorCode: row.error_code,
  errorSummary: row.error_summary,
  startedAt: row.started_at,
  completedAt: row.completed_at,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  archivedAt: row.archived_at,
})

const queryFromRow = (row: ExaEnrichmentQueryRow): ExaEnrichmentQuery => ({
  id: row.id,
  runId: row.run_id,
  assignmentId: row.assignment_id,
  queryHash: row.query_hash,
  queryText: row.query_text,
  sourceCategory: row.source_category,
  searchType: row.search_type,
  freshnessMaxAgeHours: row.freshness_max_age_hours,
  status: row.status,
  resultCount: row.result_count,
  latencyMs: row.latency_ms,
  costDollars: row.cost_dollars,
  errorCode: row.error_code,
  errorSummary: row.error_summary,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const sourceFromRow = (
  row: ExaEnrichmentSourceRow,
): ExaEnrichmentSourceCard => ({
  id: row.id,
  runId: row.run_id,
  queryId: row.query_id,
  assignmentId: row.assignment_id,
  softwareOrderId: row.software_order_id,
  siteId: row.site_id,
  sourceCategory: row.source_category,
  reviewStatus: row.review_status,
  title: row.title,
  url: row.url,
  domain: row.domain,
  publishedDate: row.published_date,
  highlightText: row.highlight_text,
  selectedTextHash: row.selected_text_hash,
  exaRequestId: row.exa_request_id,
  searchType: row.search_type,
  publicSafe: row.public_safe === 1,
  rejectedReason: row.rejected_reason,
  approvedAt: row.approved_at,
  rejectedAt: row.rejected_at,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const nullableText = (value: string | null | undefined): string | null => {
  const text = value?.trim().replace(/\s+/g, ' ')

  return text === undefined || text === '' ? null : text
}

const requiredBoundedText = (
  field: string,
  value: string,
  maxCharacters: number,
): Effect.Effect<
  string,
  | AdjutantEnrichmentLedgerUnsafePayload
  | AdjutantEnrichmentLedgerValidationError
> => {
  const text = value.trim().replace(/\s+/g, ' ')

  if (text === '') {
    return Effect.fail(
      new AdjutantEnrichmentLedgerValidationError({
        reason: `${field} is required.`,
      }),
    )
  }

  if (text.length > maxCharacters) {
    return Effect.fail(
      new AdjutantEnrichmentLedgerValidationError({
        reason: `${field} exceeds ${maxCharacters} characters.`,
      }),
    )
  }

  if (containsProviderSecretMaterial(text)) {
    return Effect.fail(
      new AdjutantEnrichmentLedgerUnsafePayload({
        reason: `${field} contains secret-shaped material.`,
      }),
    )
  }

  return Effect.succeed(text)
}

const optionalBoundedText = (
  field: string,
  value: string | null | undefined,
  maxCharacters: number,
): Effect.Effect<
  string | null,
  | AdjutantEnrichmentLedgerUnsafePayload
  | AdjutantEnrichmentLedgerValidationError
> => {
  const text = nullableText(value)

  if (text === null) {
    return Effect.succeed(null)
  }

  return requiredBoundedText(field, text, maxCharacters)
}

const assertSafeJson = (
  value: unknown,
): Effect.Effect<void, AdjutantEnrichmentLedgerUnsafePayload> => {
  const json = JSON.stringify(value)

  if (json.length > MAX_SOURCE_CARD_JSON_BYTES) {
    return Effect.fail(
      new AdjutantEnrichmentLedgerUnsafePayload({
        reason: 'Exa source-card payload is too large for D1 storage.',
      }),
    )
  }

  if (containsProviderSecretMaterial(json)) {
    return Effect.fail(
      new AdjutantEnrichmentLedgerUnsafePayload({
        reason: 'Exa source-card payload contains secret-shaped material.',
      }),
    )
  }

  return Effect.void
}

const domainForUrl = (
  value: string,
): Effect.Effect<string, AdjutantEnrichmentLedgerValidationError> =>
  Effect.try({
    catch: error =>
      new AdjutantEnrichmentLedgerValidationError({
        reason: error instanceof Error ? error.message : 'Invalid source URL.',
      }),
    try: () => new URL(value).hostname,
  })

const sha256Hex = (
  value: string,
): Effect.Effect<string, AdjutantEnrichmentLedgerStorageError> =>
  Effect.tryPromise({
    catch: error =>
      new AdjutantEnrichmentLedgerStorageError({
        operation: 'adjutantEnrichment.sha256',
        error,
      }),
    try: async () => {
      const bytes = new TextEncoder().encode(value)
      const digest = await crypto.subtle.digest('SHA-256', bytes)

      return Array.from(new Uint8Array(digest))
        .map(byte => byte.toString(16).padStart(2, '0'))
        .join('')
    },
  })

const createRun = (
  db: CrmEmailDatabase,
  runtime: AdjutantEnrichmentLedgerRuntime,
  input: CreateExaEnrichmentRunInput,
): Effect.Effect<ExaEnrichmentRun, AdjutantEnrichmentLedgerError> =>
  Effect.gen(function* () {
    const subject = yield* requiredBoundedText(
      'subject',
      input.subject,
      MAX_SUBJECT_CHARS,
    )
    const runId = runtime.makeRunId()
    const now = runtime.nowIso()
    const status = input.status ?? 'planned'

    yield* d1Effect('adjutantEnrichment.runs.insert', () =>
      crmEmailAuthorityDb(db)
        .prepare(
          `INSERT INTO exa_enrichment_runs
             (id,
              assignment_id,
              software_order_id,
              site_id,
              plan_id,
              subject,
              status,
              request_budget,
              started_at,
              created_at,
              updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          runId,
          input.assignmentId,
          input.softwareOrderId ?? null,
          input.siteId ?? null,
          input.planId,
          subject,
          status,
          Math.max(0, Math.trunc(input.requestBudget ?? 0)),
          input.startedAt ?? null,
          now,
          now,
        )
        .run(),
    )

    yield* Effect.promise(() =>
      mirrorCrmEmailRows(db, 'exa_enrichment_runs', 'id', [runId]),
    )

    return {
      id: runId,
      assignmentId: input.assignmentId,
      softwareOrderId: input.softwareOrderId ?? null,
      siteId: input.siteId ?? null,
      planId: input.planId,
      subject,
      status,
      requestBudget: Math.max(0, Math.trunc(input.requestBudget ?? 0)),
      requestCount: 0,
      cacheHitCount: 0,
      sourceCount: 0,
      approvedSourceCount: 0,
      costDollars: null,
      errorCode: null,
      errorSummary: null,
      startedAt: input.startedAt ?? null,
      completedAt: null,
      createdAt: now,
      updatedAt: now,
      archivedAt: null,
    }
  })

const recordQuery = (
  db: CrmEmailDatabase,
  runtime: AdjutantEnrichmentLedgerRuntime,
  input: RecordExaEnrichmentQueryInput,
): Effect.Effect<ExaEnrichmentQuery, AdjutantEnrichmentLedgerError> =>
  Effect.gen(function* () {
    const queryText = yield* requiredBoundedText(
      'queryText',
      input.queryText,
      MAX_QUERY_TEXT_CHARS,
    )
    const errorSummary = yield* optionalBoundedText(
      'errorSummary',
      input.errorSummary,
      MAX_ERROR_SUMMARY_CHARS,
    )
    const queryHash = yield* sha256Hex(
      `${input.sourceCategory}:${input.searchType}:${queryText}`,
    )
    const now = runtime.nowIso()
    const queryId = runtime.makeQueryId()
    const resultCount = Math.max(0, Math.trunc(input.resultCount ?? 0))
    const latencyMs =
      input.latencyMs === undefined ? null : Math.max(0, Math.trunc(input.latencyMs))

    yield* d1Effect('adjutantEnrichment.queries.insert', () =>
      crmEmailAuthorityDb(db)
        .prepare(
          `INSERT INTO exa_enrichment_queries
             (id,
              run_id,
              assignment_id,
              query_hash,
              query_text,
              source_category,
              search_type,
              freshness_max_age_hours,
              status,
              result_count,
              latency_ms,
              cost_dollars,
              error_code,
              error_summary,
              created_at,
              updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          queryId,
          input.runId,
          input.assignmentId,
          queryHash,
          queryText,
          input.sourceCategory,
          input.searchType,
          Math.max(0, Math.trunc(input.freshnessMaxAgeHours)),
          input.status ?? 'planned',
          resultCount,
          latencyMs,
          input.costDollars ?? null,
          input.errorCode ?? null,
          errorSummary,
          now,
          now,
        )
        .run(),
    )

    yield* d1Effect('adjutantEnrichment.runs.incrementRequests', () =>
      crmEmailAuthorityDb(db)
        .prepare(
          `UPDATE exa_enrichment_runs
              SET request_count = request_count + 1,
                  cache_hit_count = cache_hit_count + ?,
                  cost_dollars = COALESCE(cost_dollars, 0) + COALESCE(?, 0),
                  updated_at = ?
            WHERE id = ?`,
        )
        .bind(input.status === 'cached' ? 1 : 0, input.costDollars ?? 0, now, input.runId)
        .run(),
    )

    yield* Effect.promise(async () => {
      await mirrorCrmEmailRows(db, 'exa_enrichment_queries', 'id', [queryId])
      await mirrorCrmEmailRows(db, 'exa_enrichment_runs', 'id', [input.runId])
    })

    return {
      id: queryId,
      runId: input.runId,
      assignmentId: input.assignmentId,
      queryHash,
      queryText,
      sourceCategory: input.sourceCategory,
      searchType: input.searchType,
      freshnessMaxAgeHours: Math.max(0, Math.trunc(input.freshnessMaxAgeHours)),
      status: input.status ?? 'planned',
      resultCount,
      latencyMs,
      costDollars: input.costDollars ?? null,
      errorCode: input.errorCode ?? null,
      errorSummary,
      createdAt: now,
      updatedAt: now,
    }
  })

const refreshRunSourceCounts = (
  db: CrmEmailDatabase,
  runId: string,
  now: string,
): Effect.Effect<void, AdjutantEnrichmentLedgerStorageError> =>
  d1Effect('adjutantEnrichment.runs.refreshSourceCounts', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `UPDATE exa_enrichment_runs
            SET source_count = (
                  SELECT COUNT(*)
                    FROM exa_enrichment_sources
                   WHERE run_id = exa_enrichment_runs.id
                ),
                approved_source_count = (
                  SELECT COUNT(*)
                    FROM exa_enrichment_sources
                   WHERE run_id = exa_enrichment_runs.id
                     AND public_safe = 1
                     AND review_status IN ('approved', 'public_safe')
                ),
                updated_at = ?
          WHERE id = ?`,
      )
      .bind(now, runId)
      .run(),
  ).pipe(
    Effect.flatMap(() =>
      Effect.promise(() =>
        mirrorCrmEmailRows(db, 'exa_enrichment_runs', 'id', [runId]),
      ),
    ),
    Effect.asVoid,
  )

const storeSourceCard = (
  db: CrmEmailDatabase,
  runtime: AdjutantEnrichmentLedgerRuntime,
  input: StoreExaSourceCardInput,
): Effect.Effect<ExaEnrichmentSourceCard, AdjutantEnrichmentLedgerError> =>
  Effect.gen(function* () {
    const title = yield* requiredBoundedText(
      'title',
      input.title,
      MAX_SOURCE_TITLE_CHARS,
    )
    const url = yield* requiredBoundedText('url', input.url, MAX_SOURCE_URL_CHARS)
    const domain = yield* requiredBoundedText(
      'domain',
      input.domain ?? (yield* domainForUrl(url)),
      MAX_SOURCE_DOMAIN_CHARS,
    )
    const highlightText = yield* optionalBoundedText(
      'highlightText',
      input.highlightText,
      MAX_SOURCE_HIGHLIGHT_CHARS,
    )
    const rejectedReason = yield* optionalBoundedText(
      'rejectedReason',
      input.rejectedReason,
      MAX_ERROR_SUMMARY_CHARS,
    )
    const reviewStatus = input.reviewStatus ?? 'proposed'
    const selectedText = nullableText(input.selectedText)
    const selectedTextHash =
      selectedText === null
        ? highlightText === null
          ? null
          : yield* sha256Hex(highlightText)
        : yield* sha256Hex(selectedText)
    const sourceId = runtime.makeSourceId()
    const now = runtime.nowIso()
    const publicSafe = reviewStatus === 'public_safe'
    const approvedAt =
      reviewStatus === 'approved' || reviewStatus === 'public_safe' ? now : null
    const rejectedAt = reviewStatus === 'rejected' ? now : null
    yield* assertSafeJson({
      assignmentId: input.assignmentId,
      domain,
      exaRequestId: input.exaRequestId ?? null,
      highlightText,
      publishedDate: input.publishedDate ?? null,
      queryId: input.queryId ?? null,
      rejectedReason,
      reviewStatus,
      runId: input.runId,
      searchType: input.searchType ?? null,
      selectedTextHash,
      siteId: input.siteId ?? null,
      softwareOrderId: input.softwareOrderId ?? null,
      sourceCategory: input.sourceCategory,
      title,
      url,
    })

    yield* d1Effect('adjutantEnrichment.sources.insert', () =>
      crmEmailAuthorityDb(db)
        .prepare(
          `INSERT INTO exa_enrichment_sources
             (id,
              run_id,
              query_id,
              assignment_id,
              software_order_id,
              site_id,
              source_category,
              review_status,
              title,
              url,
              domain,
              published_date,
              highlight_text,
              selected_text_hash,
              exa_request_id,
              search_type,
              public_safe,
              rejected_reason,
              approved_at,
              rejected_at,
              created_at,
              updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          sourceId,
          input.runId,
          input.queryId ?? null,
          input.assignmentId,
          input.softwareOrderId ?? null,
          input.siteId ?? null,
          input.sourceCategory,
          reviewStatus,
          title,
          url,
          domain,
          input.publishedDate ?? null,
          highlightText,
          selectedTextHash,
          input.exaRequestId ?? null,
          input.searchType ?? null,
          publicSafe ? 1 : 0,
          rejectedReason,
          approvedAt,
          rejectedAt,
          now,
          now,
        )
        .run(),
    )

    yield* Effect.promise(() =>
      mirrorCrmEmailRows(db, 'exa_enrichment_sources', 'id', [sourceId]),
    )
    yield* refreshRunSourceCounts(db, input.runId, now)

    return {
      id: sourceId,
      runId: input.runId,
      queryId: input.queryId ?? null,
      assignmentId: input.assignmentId,
      softwareOrderId: input.softwareOrderId ?? null,
      siteId: input.siteId ?? null,
      sourceCategory: input.sourceCategory,
      reviewStatus,
      title,
      url,
      domain,
      publishedDate: input.publishedDate ?? null,
      highlightText,
      selectedTextHash,
      exaRequestId: input.exaRequestId ?? null,
      searchType: input.searchType ?? null,
      publicSafe,
      rejectedReason,
      approvedAt,
      rejectedAt,
      createdAt: now,
      updatedAt: now,
    }
  })

const reviewSourceCard = (
  db: CrmEmailDatabase,
  runtime: AdjutantEnrichmentLedgerRuntime,
  input: ReviewExaSourceCardInput,
): Effect.Effect<void, AdjutantEnrichmentLedgerError> =>
  Effect.gen(function* () {
    const now = runtime.nowIso()
    const publicSafe =
      input.publicSafe === true || input.reviewStatus === 'public_safe'
    const rejectedReason = yield* optionalBoundedText(
      'rejectedReason',
      input.rejectedReason,
      MAX_ERROR_SUMMARY_CHARS,
    )

    yield* d1Effect('adjutantEnrichment.sources.review', () =>
      crmEmailAuthorityDb(db)
        .prepare(
          `UPDATE exa_enrichment_sources
              SET review_status = ?,
                  public_safe = ?,
                  rejected_reason = ?,
                  approved_at = ?,
                  rejected_at = ?,
                  updated_at = ?
            WHERE id = ?`,
        )
        .bind(
          input.reviewStatus,
          publicSafe ? 1 : 0,
          rejectedReason,
          input.reviewStatus === 'approved' || input.reviewStatus === 'public_safe'
            ? now
            : null,
          input.reviewStatus === 'rejected' ? now : null,
          now,
          input.sourceId,
        )
        .run(),
    )

    yield* d1Effect('adjutantEnrichment.runs.refreshReviewedSourceCounts', () =>
      crmEmailAuthorityDb(db)
        .prepare(
          `UPDATE exa_enrichment_runs
              SET approved_source_count = (
                    SELECT COUNT(*)
                      FROM exa_enrichment_sources
                     WHERE run_id = exa_enrichment_runs.id
                       AND public_safe = 1
                       AND review_status IN ('approved', 'public_safe')
                  ),
                  updated_at = ?
            WHERE id = (
              SELECT run_id
                FROM exa_enrichment_sources
               WHERE id = ?
               LIMIT 1
            )`,
        )
        .bind(now, input.sourceId)
        .run(),
    )

    yield* Effect.promise(async () => {
      await mirrorCrmEmailRows(db, 'exa_enrichment_sources', 'id', [
        input.sourceId,
      ])
      const runRow = await crmEmailAuthorityDb(db)
        .prepare(
          `SELECT run_id FROM exa_enrichment_sources WHERE id = ? LIMIT 1`,
        )
        .bind(input.sourceId)
        .first<{ run_id: string }>()
      if (runRow !== null) {
        await mirrorCrmEmailRows(db, 'exa_enrichment_runs', 'id', [
          runRow.run_id,
        ])
      }
    })
  })

const linkAssignmentRun = (
  db: CrmEmailDatabase,
  runtime: AdjutantEnrichmentLedgerRuntime,
  input: LinkAssignmentEnrichmentInput,
): Effect.Effect<void, AdjutantEnrichmentLedgerError> => {
  const now = runtime.nowIso()

  return d1Effect('adjutantEnrichment.assignment.link', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `INSERT INTO adjutant_assignment_enrichments
           (assignment_id,
            enrichment_run_id,
            research_brief_id,
            status,
            required_for_launch,
            approved_at,
            created_at,
            updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(assignment_id, enrichment_run_id) DO UPDATE SET
           research_brief_id = excluded.research_brief_id,
           status = excluded.status,
           required_for_launch = excluded.required_for_launch,
           approved_at = excluded.approved_at,
           updated_at = excluded.updated_at`,
      )
      .bind(
        input.assignmentId,
        input.enrichmentRunId,
        input.researchBriefId ?? null,
        input.status,
        input.requiredForLaunch === true ? 1 : 0,
        input.status === 'approved' ? now : null,
        now,
        now,
      )
      .run(),
  ).pipe(Effect.asVoid)
}

const updateRunStatus = (
  db: CrmEmailDatabase,
  runtime: AdjutantEnrichmentLedgerRuntime,
  input: UpdateExaEnrichmentRunStatusInput,
): Effect.Effect<void, AdjutantEnrichmentLedgerError> =>
  Effect.gen(function* () {
    const now = runtime.nowIso()
    const errorSummary = yield* optionalBoundedText(
      'errorSummary',
      input.errorSummary,
      MAX_ERROR_SUMMARY_CHARS,
    )

    yield* d1Effect('adjutantEnrichment.runs.updateStatus', () =>
      crmEmailAuthorityDb(db)
        .prepare(
          `UPDATE exa_enrichment_runs
              SET status = ?,
                  error_code = ?,
                  error_summary = ?,
                  completed_at = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(
          input.status,
          input.errorCode ?? null,
          errorSummary,
          input.completedAt ?? null,
          now,
          input.runId,
        )
        .run(),
    )

    yield* Effect.promise(() =>
      mirrorCrmEmailRows(db, 'exa_enrichment_runs', 'id', [input.runId]),
    )
  })

const latestRunForAssignment = (
  db: CrmEmailDatabase,
  assignmentId: string,
): Effect.Effect<
  ExaEnrichmentRun | null,
  AdjutantEnrichmentLedgerStorageError
> =>
  d1Effect('adjutantEnrichment.runs.latestForAssignment', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                assignment_id,
                software_order_id,
                site_id,
                plan_id,
                subject,
                status,
                request_budget,
                request_count,
                cache_hit_count,
                source_count,
                approved_source_count,
                cost_dollars,
                error_code,
                error_summary,
                started_at,
                completed_at,
                created_at,
                updated_at,
                archived_at
           FROM exa_enrichment_runs
          WHERE assignment_id = ?
            AND archived_at IS NULL
          ORDER BY created_at DESC
          LIMIT 1`,
      )
      .bind(assignmentId)
      .first<ExaEnrichmentRunRow>(),
  ).pipe(Effect.map(row => (row === null ? null : runFromRow(row))))

const listQueriesForRun = (
  db: CrmEmailDatabase,
  runId: string,
): Effect.Effect<
  ReadonlyArray<ExaEnrichmentQuery>,
  AdjutantEnrichmentLedgerStorageError
> =>
  d1Effect('adjutantEnrichment.queries.listForRun', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                run_id,
                assignment_id,
                query_hash,
                query_text,
                source_category,
                search_type,
                freshness_max_age_hours,
                status,
                result_count,
                latency_ms,
                cost_dollars,
                error_code,
                error_summary,
                created_at,
                updated_at
           FROM exa_enrichment_queries
          WHERE run_id = ?
          ORDER BY created_at DESC`,
      )
      .bind(runId)
      .all<ExaEnrichmentQueryRow>(),
  ).pipe(Effect.map(result => result.results.map(queryFromRow)))

const listSourceCardsForAssignment = (
  db: CrmEmailDatabase,
  assignmentId: string,
): Effect.Effect<
  ReadonlyArray<ExaEnrichmentSourceCard>,
  AdjutantEnrichmentLedgerStorageError
> =>
  d1Effect('adjutantEnrichment.sources.listForAssignment', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                run_id,
                query_id,
                assignment_id,
                software_order_id,
                site_id,
                source_category,
                review_status,
                title,
                url,
                domain,
                published_date,
                highlight_text,
                selected_text_hash,
                exa_request_id,
                search_type,
                public_safe,
                rejected_reason,
                approved_at,
                rejected_at,
                created_at,
                updated_at
           FROM exa_enrichment_sources
          WHERE assignment_id = ?
          ORDER BY created_at DESC`,
      )
      .bind(assignmentId)
      .all<ExaEnrichmentSourceRow>(),
  ).pipe(Effect.map(result => result.results.map(sourceFromRow)))

export const publicSafeExaSourceCards = (
  sourceCards: ReadonlyArray<ExaEnrichmentSourceCard>,
): ReadonlyArray<ExaEnrichmentSourceCard> =>
  sourceCards.filter(sourceCard => {
    const json = JSON.stringify(sourceCard)

    return (
      sourceCard.publicSafe &&
      (sourceCard.reviewStatus === 'approved' ||
        sourceCard.reviewStatus === 'public_safe') &&
      !containsProviderSecretMaterial(json)
    )
  })

const publicSafeSourceCardsForAssignment = (
  db: CrmEmailDatabase,
  assignmentId: string,
): Effect.Effect<
  ReadonlyArray<ExaEnrichmentSourceCard>,
  AdjutantEnrichmentLedgerStorageError
> =>
  listSourceCardsForAssignment(db, assignmentId).pipe(
    Effect.map(publicSafeExaSourceCards),
  )

export const makeAdjutantEnrichmentLedger = (
  db: CrmEmailDatabase,
  runtime: AdjutantEnrichmentLedgerRuntime =
    systemAdjutantEnrichmentLedgerRuntime,
) => ({
  createRun: Effect.fn('AdjutantEnrichmentLedger.createRun')(
    (input: CreateExaEnrichmentRunInput) => createRun(db, runtime, input),
  ),
  latestRunForAssignment: Effect.fn(
    'AdjutantEnrichmentLedger.latestRunForAssignment',
  )((assignmentId: string) => latestRunForAssignment(db, assignmentId)),
  queriesForRun: Effect.fn('AdjutantEnrichmentLedger.queriesForRun')(
    (runId: string) => listQueriesForRun(db, runId),
  ),
  linkAssignmentRun: Effect.fn(
    'AdjutantEnrichmentLedger.linkAssignmentRun',
  )((input: LinkAssignmentEnrichmentInput) =>
    linkAssignmentRun(db, runtime, input),
  ),
  publicSafeSourceCardsForAssignment: Effect.fn(
    'AdjutantEnrichmentLedger.publicSafeSourceCardsForAssignment',
  )((assignmentId: string) => publicSafeSourceCardsForAssignment(db, assignmentId)),
  recordQuery: Effect.fn('AdjutantEnrichmentLedger.recordQuery')(
    (input: RecordExaEnrichmentQueryInput) =>
      recordQuery(db, runtime, input),
  ),
  reviewSourceCard: Effect.fn('AdjutantEnrichmentLedger.reviewSourceCard')(
    (input: ReviewExaSourceCardInput) => reviewSourceCard(db, runtime, input),
  ),
  sourceCardsForAssignment: Effect.fn(
    'AdjutantEnrichmentLedger.sourceCardsForAssignment',
  )((assignmentId: string) => listSourceCardsForAssignment(db, assignmentId)),
  storeSourceCard: Effect.fn('AdjutantEnrichmentLedger.storeSourceCard')(
    (input: StoreExaSourceCardInput) => storeSourceCard(db, runtime, input),
  ),
  updateRunStatus: Effect.fn('AdjutantEnrichmentLedger.updateRunStatus')(
    (input: UpdateExaEnrichmentRunStatusInput) =>
      updateRunStatus(db, runtime, input),
  ),
})

export class AdjutantEnrichmentLedger extends Context.Service<
  AdjutantEnrichmentLedger,
  ReturnType<typeof makeAdjutantEnrichmentLedger>
>()('@openagentsinc/autopilot-omega/AdjutantEnrichmentLedger') {
  static layer = (
    env: AdjutantEnrichmentLedgerEnv,
    runtime: AdjutantEnrichmentLedgerRuntime =
      systemAdjutantEnrichmentLedgerRuntime,
  ) =>
    Layer.succeed(
      AdjutantEnrichmentLedger,
      makeAdjutantEnrichmentLedger(openAgentsDatabase(env), runtime),
    )
}
