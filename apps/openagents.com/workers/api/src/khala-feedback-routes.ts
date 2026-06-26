// Khala feedback route.
//
//   POST /api/khala/feedback
//   GET  /api/operator/khala/feedback
//
// The public route is intentionally tiny and unauthenticated so the npm CLI can
// submit frustration/quality notes without routing slash commands through
// inference. Operator reads stay admin-token gated and bounded.

import { Effect, Schema as S } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

const KHALA_FEEDBACK_MAX_CHARS = 4_000
const KHALA_FEEDBACK_MAX_LIMIT = 100

const KhalaFeedbackSubmitBody = S.Struct({
  feedback: S.optional(S.String),
  text: S.optional(S.String),
  traceRef: S.optional(S.String),
  source: S.optional(S.String),
  clientVersion: S.optional(S.String),
})

export type KhalaFeedbackSubmitBody = typeof KhalaFeedbackSubmitBody.Type

type KhalaFeedbackRow = Readonly<{
  feedback_ref: string
  trace_ref: string | null
  feedback_text: string
  source: string
  client_version: string | null
  user_agent: string | null
  created_at: string
}>

export type KhalaFeedbackRecord = Readonly<{
  feedbackRef: string
  traceRef: string | null
  feedback: string
  source: string
  clientVersion: string | null
  userAgent: string | null
  createdAt: string
}>

export type KhalaFeedbackCreateInput = Readonly<{
  feedbackRef: string
  traceRef: string | null
  feedback: string
  source: string
  clientVersion: string | null
  userAgent: string | null
  createdAt: string
}>

export type KhalaFeedbackListInput = Readonly<{
  limit: number
  traceRef?: string | undefined
}>

export type KhalaFeedbackStore = Readonly<{
  create: (input: KhalaFeedbackCreateInput) => Promise<KhalaFeedbackRecord>
  listRecent: (
    input: KhalaFeedbackListInput,
  ) => Promise<ReadonlyArray<KhalaFeedbackRecord>>
}>

export type KhalaFeedbackSubmitDependencies = Readonly<{
  store: KhalaFeedbackStore
  makeFeedbackRef?: (() => string) | undefined
  nowIso?: (() => string) | undefined
}>

export type OperatorKhalaFeedbackDependencies = Readonly<{
  store: KhalaFeedbackStore
  requireAdminApiToken: (request: Request) => Promise<boolean>
}>

class KhalaFeedbackBadRequest extends S.TaggedErrorClass<KhalaFeedbackBadRequest>()(
  'KhalaFeedbackBadRequest',
  {
    reason: S.String,
  },
) {}

class KhalaFeedbackStorageError extends S.TaggedErrorClass<KhalaFeedbackStorageError>()(
  'KhalaFeedbackStorageError',
  {
    reason: S.String,
  },
) {}

const cleanOptionalString = (value: string | undefined): string | null => {
  const cleaned = value?.trim()
  return cleaned === undefined || cleaned.length === 0 ? null : cleaned
}

const normalizeFeedbackBody = (
  body: KhalaFeedbackSubmitBody,
): Effect.Effect<
  Readonly<{
    feedback: string
    traceRef: string | null
    source: string
    clientVersion: string | null
  }>,
  KhalaFeedbackBadRequest
> => {
  const feedback = (body.feedback ?? body.text ?? '').trim()
  if (feedback.length === 0) {
    return new KhalaFeedbackBadRequest({
      reason: 'feedback must be a non-empty string',
    })
  }
  if (feedback.length > KHALA_FEEDBACK_MAX_CHARS) {
    return new KhalaFeedbackBadRequest({
      reason: `feedback must be ${KHALA_FEEDBACK_MAX_CHARS} characters or fewer`,
    })
  }

  const source = cleanOptionalString(body.source) ?? 'khala-cli'
  const traceRef = cleanOptionalString(body.traceRef)
  const clientVersion = cleanOptionalString(body.clientVersion)

  return Effect.succeed({ clientVersion, feedback, source, traceRef })
}

const decodeSubmitBody = (
  request: Request,
): Effect.Effect<KhalaFeedbackSubmitBody, KhalaFeedbackBadRequest> =>
  Effect.gen(function* () {
    const payload = yield* Effect.tryPromise({
      try: () => request.json(),
      catch: error =>
        new KhalaFeedbackBadRequest({
          reason: error instanceof Error ? error.message : 'invalid json',
        }),
    })

    return yield* S.decodeUnknownEffect(KhalaFeedbackSubmitBody)(payload).pipe(
      Effect.mapError(error => new KhalaFeedbackBadRequest({ reason: String(error) })),
    )
  })

const feedbackRef = (makeFeedbackRef?: () => string): string =>
  makeFeedbackRef?.() ?? `khala_feedback:${compactRandomId('fb')}`

const rowToRecord = (row: KhalaFeedbackRow): KhalaFeedbackRecord => ({
  clientVersion: row.client_version,
  createdAt: row.created_at,
  feedback: row.feedback_text,
  feedbackRef: row.feedback_ref,
  source: row.source,
  traceRef: row.trace_ref,
  userAgent: row.user_agent,
})

export const makeD1KhalaFeedbackStore = (
  db: D1Database,
): KhalaFeedbackStore => ({
  create: async input => {
    await db
      .prepare(
        `INSERT INTO khala_feedback (
            feedback_ref,
            trace_ref,
            feedback_text,
            source,
            client_version,
            user_agent,
            created_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        input.feedbackRef,
        input.traceRef,
        input.feedback,
        input.source,
        input.clientVersion,
        input.userAgent,
        input.createdAt,
      )
      .run()

    return {
      clientVersion: input.clientVersion,
      createdAt: input.createdAt,
      feedback: input.feedback,
      feedbackRef: input.feedbackRef,
      source: input.source,
      traceRef: input.traceRef,
      userAgent: input.userAgent,
    }
  },

  listRecent: async input => {
    const limit = Math.min(Math.max(Math.trunc(input.limit), 1), KHALA_FEEDBACK_MAX_LIMIT)
    const traceRef = cleanOptionalString(input.traceRef)
    const query =
      traceRef === null
        ? db
            .prepare(
              `SELECT feedback_ref,
                      trace_ref,
                      feedback_text,
                      source,
                      client_version,
                      user_agent,
                      created_at
                 FROM khala_feedback
                ORDER BY created_at DESC
                LIMIT ?`,
            )
            .bind(limit)
        : db
            .prepare(
              `SELECT feedback_ref,
                      trace_ref,
                      feedback_text,
                      source,
                      client_version,
                      user_agent,
                      created_at
                 FROM khala_feedback
                WHERE trace_ref = ?
                ORDER BY created_at DESC
                LIMIT ?`,
            )
            .bind(traceRef, limit)

    const rows = await query.all<KhalaFeedbackRow>()
    return rows.results.map(rowToRecord)
  },
})

export const handleKhalaFeedbackSubmit = (
  request: Request,
  dependencies: KhalaFeedbackSubmitDependencies,
): Effect.Effect<HttpResponse> => {
  if (request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['POST']))
  }

  const nowIso = dependencies.nowIso ?? currentIsoTimestamp

  return decodeSubmitBody(request).pipe(
    Effect.flatMap(normalizeFeedbackBody),
    Effect.flatMap(body =>
      Effect.tryPromise({
        try: () =>
          dependencies.store.create({
            clientVersion: body.clientVersion,
            createdAt: nowIso(),
            feedback: body.feedback,
            feedbackRef: feedbackRef(dependencies.makeFeedbackRef),
            source: body.source,
            traceRef: body.traceRef,
            userAgent: request.headers.get('user-agent'),
          }),
        catch: error =>
          new KhalaFeedbackStorageError({
            reason: error instanceof Error ? error.message : String(error),
          }),
      }),
    ),
    Effect.map(record =>
      noStoreJsonResponse(
        {
          schemaVersion: 'openagents.khala.feedback.submit.v1',
          createdAt: record.createdAt,
          feedbackRef: record.feedbackRef,
          traceRef: record.traceRef,
        },
        { status: 201 },
      ),
    ),
    Effect.catchTags({
      KhalaFeedbackBadRequest: ({ reason }) =>
        Effect.succeed(
          noStoreJsonResponse(
            { error: 'bad_request', reason },
            { status: 400 },
          ),
        ),
      KhalaFeedbackStorageError: () =>
        Effect.succeed(
          noStoreJsonResponse(
            { error: 'feedback_storage_unavailable' },
            { status: 500 },
          ),
        ),
    }),
  )
}

export const handleOperatorKhalaFeedback = (
  request: Request,
  dependencies: OperatorKhalaFeedbackDependencies,
): Effect.Effect<HttpResponse> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }

  return Effect.gen(function* () {
    const authorized = yield* Effect.tryPromise({
      try: () => dependencies.requireAdminApiToken(request),
      catch: () => false,
    })
    if (!authorized) {
      return unauthorized()
    }

    const url = new URL(request.url)
    const requestedLimit = Number(url.searchParams.get('limit') ?? '50')
    const limit = Number.isFinite(requestedLimit)
      ? Math.min(Math.max(Math.trunc(requestedLimit), 1), KHALA_FEEDBACK_MAX_LIMIT)
      : 50
    const traceRef = cleanOptionalString(url.searchParams.get('traceRef') ?? undefined)
    const feedback = yield* Effect.tryPromise({
      try: () =>
        dependencies.store.listRecent({
          limit,
          ...(traceRef === null ? {} : { traceRef }),
        }),
      catch: error =>
        new KhalaFeedbackStorageError({
          reason: error instanceof Error ? error.message : String(error),
        }),
    })

    return noStoreJsonResponse({
      schemaVersion: 'openagents.khala.feedback.list.v1',
      feedback,
    })
  }).pipe(
    Effect.catchTag('KhalaFeedbackStorageError', () =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'feedback_storage_unavailable' },
          { status: 500 },
        ),
      ),
    ),
  )
}
