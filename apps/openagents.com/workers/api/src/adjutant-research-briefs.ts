import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import type { ExaEnrichmentSourceCard } from './adjutant-enrichment-ledger'
import { parseJsonStringArray, parseJsonWithSchema } from './json-boundary'
import { openAgentsDatabase } from './runtime'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

type AdjutantResearchBriefEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

export type AdjutantResearchBriefRuntime = Readonly<{
  makeBriefId: () => string
  nowIso: () => string
}>

export const systemAdjutantResearchBriefRuntime: AdjutantResearchBriefRuntime =
  {
    makeBriefId: () => compactRandomId('adjutant_research_brief'),
    nowIso: currentIsoTimestamp,
  }

const MAX_FACTS = 8
const MAX_SECTIONS = 8
const MAX_SOURCES = 12
const MAX_TEXT_CHARS = 1200
const MAX_ITEM_CHARS = 280
const MAX_REVIEW_REASON_CHARS = 500

export const AdjutantResearchBriefStatus = S.Literals([
  'draft',
  'needs_review',
  'approved',
  'rejected',
  'stale',
])
export type AdjutantResearchBriefStatus =
  typeof AdjutantResearchBriefStatus.Type

export const AdjutantResearchBriefSourceCard = S.Struct({
  id: S.String,
  title: S.String,
  url: S.String,
  domain: S.String,
  highlightText: S.NullOr(S.String),
})
export type AdjutantResearchBriefSourceCard =
  typeof AdjutantResearchBriefSourceCard.Type

export const AdjutantResearchBrief = S.Struct({
  id: S.String,
  assignmentId: S.String,
  enrichmentRunId: S.NullOr(S.String),
  status: AdjutantResearchBriefStatus,
  summary: S.String,
  groundedFacts: S.Array(S.String),
  suggestedSections: S.Array(S.String),
  unknowns: S.Array(S.String),
  claimsNeedingReview: S.Array(S.String),
  sourceCards: S.Array(AdjutantResearchBriefSourceCard),
  createdByUserId: S.NullOr(S.String),
  reviewedByUserId: S.NullOr(S.String),
  reviewReason: S.NullOr(S.String),
  approvedAt: S.NullOr(S.String),
  rejectedAt: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
  archivedAt: S.NullOr(S.String),
})
export type AdjutantResearchBrief = typeof AdjutantResearchBrief.Type

export type CreateAdjutantResearchBriefInput = Readonly<{
  assignmentId: string
  claimsNeedingReview?: ReadonlyArray<string> | undefined
  createdByUserId?: string | null | undefined
  customerRequest: string
  enrichmentRunId?: string | null | undefined
  sourceCards: ReadonlyArray<ExaEnrichmentSourceCard>
  status?: AdjutantResearchBriefStatus | undefined
  suggestedSections?: ReadonlyArray<string> | undefined
  unknowns?: ReadonlyArray<string> | undefined
}>

export const ReviewAdjutantResearchBriefInput = S.Struct({
  briefId: S.String,
  reviewReason: S.optionalKey(S.NullOr(S.String)),
  reviewedByUserId: S.optionalKey(S.NullOr(S.String)),
  status: S.Literals(['approved', 'rejected', 'stale']),
})
export type ReviewAdjutantResearchBriefInput =
  typeof ReviewAdjutantResearchBriefInput.Type

type AdjutantResearchBriefRow = Readonly<{
  approved_at: string | null
  archived_at: string | null
  assignment_id: string
  claims_needing_review_json: string
  created_at: string
  created_by_user_id: string | null
  enrichment_run_id: string | null
  grounded_facts_json: string
  id: string
  rejected_at: string | null
  review_reason: string | null
  reviewed_by_user_id: string | null
  source_cards_json: string
  status: AdjutantResearchBriefStatus
  suggested_sections_json: string
  summary: string
  unknowns_json: string
  updated_at: string
}>

export class AdjutantResearchBriefStorageError extends S.TaggedErrorClass<AdjutantResearchBriefStorageError>()(
  'AdjutantResearchBriefStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class AdjutantResearchBriefUnsafePayload extends S.TaggedErrorClass<AdjutantResearchBriefUnsafePayload>()(
  'AdjutantResearchBriefUnsafePayload',
  {
    reason: S.String,
  },
) {}

export class AdjutantResearchBriefValidationError extends S.TaggedErrorClass<AdjutantResearchBriefValidationError>()(
  'AdjutantResearchBriefValidationError',
  {
    reason: S.String,
  },
) {}

export type AdjutantResearchBriefError =
  | AdjutantResearchBriefStorageError
  | AdjutantResearchBriefUnsafePayload
  | AdjutantResearchBriefValidationError

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AdjutantResearchBriefStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new AdjutantResearchBriefStorageError({ operation, error }),
  })

const parseStringArray = (value: string): ReadonlyArray<string> =>
  parseJsonStringArray(value)

const parseSourceCards = (
  value: string,
): ReadonlyArray<AdjutantResearchBriefSourceCard> =>
  parseJsonWithSchema(S.Array(AdjutantResearchBriefSourceCard), value)

const briefFromRow = (row: AdjutantResearchBriefRow): AdjutantResearchBrief => ({
  id: row.id,
  assignmentId: row.assignment_id,
  enrichmentRunId: row.enrichment_run_id,
  status: row.status,
  summary: row.summary,
  groundedFacts: [...parseStringArray(row.grounded_facts_json)],
  suggestedSections: [...parseStringArray(row.suggested_sections_json)],
  unknowns: [...parseStringArray(row.unknowns_json)],
  claimsNeedingReview: [...parseStringArray(row.claims_needing_review_json)],
  sourceCards: [...parseSourceCards(row.source_cards_json)],
  createdByUserId: row.created_by_user_id,
  reviewedByUserId: row.reviewed_by_user_id,
  reviewReason: row.review_reason,
  approvedAt: row.approved_at,
  rejectedAt: row.rejected_at,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  archivedAt: row.archived_at,
})

const normalizedText = (value: string): string =>
  value.trim().replace(/\s+/g, ' ')

const boundedText = (
  field: string,
  value: string,
  maxCharacters: number,
): Effect.Effect<
  string,
  AdjutantResearchBriefUnsafePayload | AdjutantResearchBriefValidationError
> => {
  const text = normalizedText(value)

  if (text === '') {
    return Effect.fail(
      new AdjutantResearchBriefValidationError({
        reason: `${field} is required.`,
      }),
    )
  }

  if (text.length > maxCharacters) {
    return Effect.fail(
      new AdjutantResearchBriefValidationError({
        reason: `${field} exceeds ${maxCharacters} characters.`,
      }),
    )
  }

  if (containsProviderSecretMaterial(text)) {
    return Effect.fail(
      new AdjutantResearchBriefUnsafePayload({
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
  AdjutantResearchBriefUnsafePayload | AdjutantResearchBriefValidationError
> => {
  const text = value === null || value === undefined ? '' : normalizedText(value)

  return text === ''
    ? Effect.succeed(null)
    : boundedText(field, text, maxCharacters)
}

const boundedStringArray = (
  field: string,
  values: ReadonlyArray<string>,
  limit: number,
): Effect.Effect<
  ReadonlyArray<string>,
  AdjutantResearchBriefUnsafePayload | AdjutantResearchBriefValidationError
> =>
  Effect.all(
    values.slice(0, limit).map(value => boundedText(field, value, MAX_ITEM_CHARS)),
  )

const jsonForStorage = (
  field: string,
  value: unknown,
): Effect.Effect<string, AdjutantResearchBriefUnsafePayload> =>
  Effect.gen(function* () {
    const json = JSON.stringify(value)

    if (containsProviderSecretMaterial(json)) {
      return yield* new AdjutantResearchBriefUnsafePayload({
        reason: `${field} contains secret-shaped material.`,
      })
    }

    return json
  })

const sourceCardForBrief = (
  sourceCard: ExaEnrichmentSourceCard,
): AdjutantResearchBriefSourceCard => ({
  id: sourceCard.id,
  title: sourceCard.title,
  url: sourceCard.url,
  domain: sourceCard.domain,
  highlightText: sourceCard.highlightText,
})

const factTextFromSourceHighlight = (value: string): string =>
  value.length <= MAX_ITEM_CHARS ? value : value.slice(0, MAX_ITEM_CHARS)

const defaultFactsFromSources = (
  sourceCards: ReadonlyArray<ExaEnrichmentSourceCard>,
): ReadonlyArray<string> =>
  sourceCards
    .filter(
      sourceCard =>
        sourceCard.publicSafe &&
        (sourceCard.reviewStatus === 'approved' ||
          sourceCard.reviewStatus === 'public_safe'),
    )
    .flatMap(sourceCard =>
      sourceCard.highlightText === null
        ? [`${sourceCard.title} is an approved public source for this assignment.`]
        : [factTextFromSourceHighlight(sourceCard.highlightText)],
    )
    .slice(0, MAX_FACTS)

const defaultSourceCards = (
  sourceCards: ReadonlyArray<ExaEnrichmentSourceCard>,
): ReadonlyArray<AdjutantResearchBriefSourceCard> =>
  sourceCards
    .filter(
      sourceCard =>
        sourceCard.publicSafe &&
        (sourceCard.reviewStatus === 'approved' ||
          sourceCard.reviewStatus === 'public_safe'),
    )
    .slice(0, MAX_SOURCES)
    .map(sourceCardForBrief)

const defaultSections = (
  customerRequest: string,
  sourceCards: ReadonlyArray<AdjutantResearchBriefSourceCard>,
): ReadonlyArray<string> =>
  sourceCards.length === 0
    ? ['Ground the page in the customer request and mark unsupported claims for review.']
    : [
        'Lead with the customer request and the strongest sourced public evidence.',
        'Add source-grounded sections for technical context, customer relevance, and next steps.',
      ].concat(
        customerRequest.toLowerCase().includes('otec')
          ? ['Include a sourced OTEC/SWAC infrastructure context section.']
          : [],
      )

const createBrief = (
  db: D1Database,
  runtime: AdjutantResearchBriefRuntime,
  input: CreateAdjutantResearchBriefInput,
): Effect.Effect<AdjutantResearchBrief, AdjutantResearchBriefError> =>
  Effect.gen(function* () {
    const approvedSources = defaultSourceCards(input.sourceCards)
    const facts = yield* boundedStringArray(
      'groundedFact',
      defaultFactsFromSources(input.sourceCards),
      MAX_FACTS,
    )
    const suggestedSections = yield* boundedStringArray(
      'suggestedSection',
      input.suggestedSections ??
        defaultSections(input.customerRequest, approvedSources),
      MAX_SECTIONS,
    )
    const unknowns = yield* boundedStringArray(
      'unknown',
      input.unknowns ?? [
        'Confirm any customer-specific claims that are not directly supported by approved public sources.',
      ],
      MAX_SECTIONS,
    )
    const claimsNeedingReview = yield* boundedStringArray(
      'claimNeedingReview',
      input.claimsNeedingReview ??
        (approvedSources.length === 0
          ? ['No approved public sources are available for this brief yet.']
          : []),
      MAX_SECTIONS,
    )
    const summary = yield* boundedText(
      'summary',
      facts.length === 0
        ? `Research brief for: ${input.customerRequest}`
        : facts.slice(0, 3).join(' '),
      MAX_TEXT_CHARS,
    )
    const sourceCardsJson = yield* jsonForStorage(
      'sourceCards',
      approvedSources,
    )
    const groundedFactsJson = yield* jsonForStorage('groundedFacts', facts)
    const suggestedSectionsJson = yield* jsonForStorage(
      'suggestedSections',
      suggestedSections,
    )
    const unknownsJson = yield* jsonForStorage('unknowns', unknowns)
    const claimsNeedingReviewJson = yield* jsonForStorage(
      'claimsNeedingReview',
      claimsNeedingReview,
    )
    const now = runtime.nowIso()
    const status = input.status ?? 'needs_review'
    const approvedAt = status === 'approved' ? now : null
    const rejectedAt = status === 'rejected' ? now : null
    const briefId = runtime.makeBriefId()

    yield* d1Effect('adjutantResearchBriefs.insert', () =>
      db
        .prepare(
          `INSERT INTO adjutant_research_briefs
             (id,
              assignment_id,
              enrichment_run_id,
              status,
              summary,
              grounded_facts_json,
              suggested_sections_json,
              unknowns_json,
              claims_needing_review_json,
              source_cards_json,
              created_by_user_id,
              approved_at,
              rejected_at,
              created_at,
              updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          briefId,
          input.assignmentId,
          input.enrichmentRunId ?? null,
          status,
          summary,
          groundedFactsJson,
          suggestedSectionsJson,
          unknownsJson,
          claimsNeedingReviewJson,
          sourceCardsJson,
          input.createdByUserId ?? null,
          approvedAt,
          rejectedAt,
          now,
          now,
        )
        .run(),
    )

    return {
      id: briefId,
      assignmentId: input.assignmentId,
      enrichmentRunId: input.enrichmentRunId ?? null,
      status,
      summary,
      groundedFacts: [...facts],
      suggestedSections: [...suggestedSections],
      unknowns: [...unknowns],
      claimsNeedingReview: [...claimsNeedingReview],
      sourceCards: [...approvedSources],
      createdByUserId: input.createdByUserId ?? null,
      reviewedByUserId: null,
      reviewReason: null,
      approvedAt,
      rejectedAt,
      createdAt: now,
      updatedAt: now,
      archivedAt: null,
    }
  })

const reviewBrief = (
  db: D1Database,
  runtime: AdjutantResearchBriefRuntime,
  input: ReviewAdjutantResearchBriefInput,
): Effect.Effect<void, AdjutantResearchBriefError> =>
  Effect.gen(function* () {
    const reason = yield* optionalBoundedText(
      'reviewReason',
      input.reviewReason,
      MAX_REVIEW_REASON_CHARS,
    )
    const now = runtime.nowIso()

    yield* d1Effect('adjutantResearchBriefs.review', () =>
      db
        .prepare(
          `UPDATE adjutant_research_briefs
              SET status = ?,
                  reviewed_by_user_id = ?,
                  review_reason = ?,
                  approved_at = ?,
                  rejected_at = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(
          input.status,
          input.reviewedByUserId ?? null,
          reason,
          input.status === 'approved' ? now : null,
          input.status === 'rejected' ? now : null,
          now,
          input.briefId,
        )
        .run(),
    )
  })

const readLatestBriefForAssignment = (
  db: D1Database,
  assignmentId: string,
  status?: AdjutantResearchBriefStatus | undefined,
): Effect.Effect<
  AdjutantResearchBrief | null,
  AdjutantResearchBriefStorageError
> =>
  d1Effect('adjutantResearchBriefs.latestForAssignment', () =>
    db
      .prepare(
        `SELECT id,
                assignment_id,
                enrichment_run_id,
                status,
                summary,
                grounded_facts_json,
                suggested_sections_json,
                unknowns_json,
                claims_needing_review_json,
                source_cards_json,
                created_by_user_id,
                reviewed_by_user_id,
                review_reason,
                approved_at,
                rejected_at,
                created_at,
                updated_at,
                archived_at
           FROM adjutant_research_briefs
          WHERE assignment_id = ?
            AND archived_at IS NULL
            AND (? IS NULL OR status = ?)
          ORDER BY updated_at DESC
          LIMIT 1`,
      )
      .bind(assignmentId, status ?? null, status ?? null)
      .first<AdjutantResearchBriefRow>(),
  ).pipe(Effect.map(row => (row === null ? null : briefFromRow(row))))

export const makeAdjutantResearchBriefService = (
  db: D1Database,
  runtime: AdjutantResearchBriefRuntime =
    systemAdjutantResearchBriefRuntime,
) => ({
  createBrief: Effect.fn('AdjutantResearchBriefService.createBrief')(
    (input: CreateAdjutantResearchBriefInput) =>
      createBrief(db, runtime, input),
  ),
  latestApprovedBriefForAssignment: Effect.fn(
    'AdjutantResearchBriefService.latestApprovedBriefForAssignment',
  )((assignmentId: string) =>
    readLatestBriefForAssignment(db, assignmentId, 'approved'),
  ),
  latestBriefForAssignment: Effect.fn(
    'AdjutantResearchBriefService.latestBriefForAssignment',
  )((assignmentId: string) => readLatestBriefForAssignment(db, assignmentId)),
  reviewBrief: Effect.fn('AdjutantResearchBriefService.reviewBrief')(
    (input: ReviewAdjutantResearchBriefInput) =>
      reviewBrief(db, runtime, input),
  ),
})

export class AdjutantResearchBriefService extends Context.Service<
  AdjutantResearchBriefService,
  ReturnType<typeof makeAdjutantResearchBriefService>
>()('@openagentsinc/autopilot-omega/AdjutantResearchBriefService') {
  static layer = (
    env: AdjutantResearchBriefEnv,
    runtime: AdjutantResearchBriefRuntime =
      systemAdjutantResearchBriefRuntime,
  ) =>
    Layer.succeed(
      AdjutantResearchBriefService,
      makeAdjutantResearchBriefService(openAgentsDatabase(env), runtime),
    )
}
