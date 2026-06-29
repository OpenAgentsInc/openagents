// Khala unsupported-request operator route (#6357).
//
//   GET  /api/operator/khala/unsupported-requests
//   POST /api/operator/khala/unsupported-requests
//
// Admin scoped. Maintains the running list of what testers try that Khala cannot
// do yet. Rows are bounded summaries and refs only; raw traces, raw feedback
// transcripts, private paths, and provider payloads stay out of this ledger.
import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { parseJsonStringArray } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

type HttpResponse = globalThis.Response

const KHALA_UNSUPPORTED_REQUEST_MAX_LIMIT = 100
const KHALA_UNSUPPORTED_REQUEST_MAX_TITLE_CHARS = 180
const KHALA_UNSUPPORTED_REQUEST_MAX_SUMMARY_CHARS = 1_200
const KHALA_UNSUPPORTED_REQUEST_MAX_REFS = 20
const KHALA_UNSUPPORTED_REQUEST_MAX_REF_CHARS = 260

const KhalaUnsupportedRequestSourceKind = S.Literals([
  'trace_review',
  'khala_feedback',
  'forum',
  'operator',
])
export type KhalaUnsupportedRequestSourceKind =
  typeof KhalaUnsupportedRequestSourceKind.Type

const KhalaUnsupportedRequestTriageKind = S.Literals([
  'needs_triage',
  'bug',
  'missing_capability',
  'wont_do',
])
export type KhalaUnsupportedRequestTriageKind =
  typeof KhalaUnsupportedRequestTriageKind.Type

const KhalaUnsupportedRequestStatus = S.Literals([
  'open',
  'needs_issue',
  'issue_opened',
  'closed',
  'wont_do',
])
export type KhalaUnsupportedRequestStatus =
  typeof KhalaUnsupportedRequestStatus.Type

const KhalaUnsupportedRequestCreateBody = S.Struct({
  evidenceRefs: S.optional(S.Array(S.String)),
  forumTopicRef: S.optional(S.String),
  githubIssueRef: S.optional(S.String),
  requestRef: S.optional(S.String),
  sourceKind: S.optional(KhalaUnsupportedRequestSourceKind),
  sourceRef: S.String,
  status: S.optional(KhalaUnsupportedRequestStatus),
  suggestedIssueTitle: S.optional(S.String),
  summary: S.optional(S.String),
  title: S.String,
  triageKind: S.optional(KhalaUnsupportedRequestTriageKind),
})

export type KhalaUnsupportedRequestCreateBody =
  typeof KhalaUnsupportedRequestCreateBody.Type

type KhalaUnsupportedRequestRow = Readonly<{
  request_ref: string
  source_kind: KhalaUnsupportedRequestSourceKind
  source_ref: string
  title: string
  summary: string
  triage_kind: KhalaUnsupportedRequestTriageKind
  status: KhalaUnsupportedRequestStatus
  forum_topic_ref: string | null
  github_issue_ref: string | null
  evidence_refs_json: string
  suggested_issue_title: string
  created_at: string
  updated_at: string
}>

export type KhalaUnsupportedRequestRecord = Readonly<{
  requestRef: string
  sourceKind: KhalaUnsupportedRequestSourceKind
  sourceRef: string
  title: string
  summary: string
  triageKind: KhalaUnsupportedRequestTriageKind
  status: KhalaUnsupportedRequestStatus
  forumTopicRef: string | null
  githubIssueRef: string | null
  evidenceRefs: ReadonlyArray<string>
  suggestedIssueTitle: string
  issueRequired: boolean
  nextAction: 'triage' | 'link_forum_report' | 'open_github_issue' | 'none'
  createdAt: string
  updatedAt: string
}>

export type KhalaUnsupportedRequestCreateInput = Readonly<{
  requestRef: string
  sourceKind: KhalaUnsupportedRequestSourceKind
  sourceRef: string
  title: string
  summary: string
  triageKind: KhalaUnsupportedRequestTriageKind
  status: KhalaUnsupportedRequestStatus
  forumTopicRef: string | null
  githubIssueRef: string | null
  evidenceRefs: ReadonlyArray<string>
  suggestedIssueTitle: string
  createdAt: string
  updatedAt: string
}>

export type KhalaUnsupportedRequestListInput = Readonly<{
  limit: number
  sourceKind?: KhalaUnsupportedRequestSourceKind | undefined
  status?: KhalaUnsupportedRequestStatus | undefined
  triageKind?: KhalaUnsupportedRequestTriageKind | undefined
}>

export type KhalaUnsupportedRequestStore = Readonly<{
  upsert: (
    input: KhalaUnsupportedRequestCreateInput,
  ) => Promise<KhalaUnsupportedRequestRecord>
  listRecent: (
    input: KhalaUnsupportedRequestListInput,
  ) => Promise<ReadonlyArray<KhalaUnsupportedRequestRecord>>
}>

export type OperatorKhalaUnsupportedRequestDependencies = Readonly<{
  requireAdminApiToken: (request: Request) => Promise<boolean>
  store: KhalaUnsupportedRequestStore
  makeRequestRef?: (() => string) | undefined
  nowIso?: (() => string) | undefined
}>

class KhalaUnsupportedRequestBadRequest extends S.TaggedErrorClass<KhalaUnsupportedRequestBadRequest>()(
  'KhalaUnsupportedRequestBadRequest',
  {
    reason: S.String,
  },
) {}

class KhalaUnsupportedRequestStorageError extends S.TaggedErrorClass<KhalaUnsupportedRequestStorageError>()(
  'KhalaUnsupportedRequestStorageError',
  {
    reason: S.String,
  },
) {}

const cleanOptionalString = (value: string | undefined): string | null => {
  const cleaned = value?.trim()
  return cleaned === undefined || cleaned.length === 0 ? null : cleaned
}

const publicRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/#-]{0,259}$/
const localPathPattern =
  /(^|[\s"'`])(\/Users\/|\/home\/|\/private\/|[A-Za-z]:\\)[^\s"'`]*/u

const isPublicSafeRef = (value: string): boolean =>
  value.length <= KHALA_UNSUPPORTED_REQUEST_MAX_REF_CHARS &&
  publicRefPattern.test(value) &&
  !value.includes('..') &&
  !localPathPattern.test(value)

const rejectPrivateShapedValue = (
  value: unknown,
): Effect.Effect<void, KhalaUnsupportedRequestBadRequest> => {
  const serialized = JSON.stringify(value)
  if (
    containsProviderSecretMaterial(serialized) ||
    localPathPattern.test(serialized)
  ) {
    return new KhalaUnsupportedRequestBadRequest({
      reason:
        'unsupported request entries must contain public-safe summaries and refs only',
    })
  }
  return Effect.void
}

const cleanRequiredText = (
  value: string,
  field: string,
  maxChars: number,
): Effect.Effect<string, KhalaUnsupportedRequestBadRequest> => {
  const cleaned = value.trim()
  if (cleaned.length === 0) {
    return new KhalaUnsupportedRequestBadRequest({
      reason: `${field} must be a non-empty string`,
    })
  }
  if (cleaned.length > maxChars) {
    return new KhalaUnsupportedRequestBadRequest({
      reason: `${field} must be ${maxChars} characters or fewer`,
    })
  }
  return rejectPrivateShapedValue(cleaned).pipe(Effect.as(cleaned))
}

const cleanOptionalText = (
  value: string | undefined,
  field: string,
  maxChars: number,
): Effect.Effect<string | null, KhalaUnsupportedRequestBadRequest> => {
  const cleaned = cleanOptionalString(value)
  if (cleaned === null) {
    return Effect.succeed(null)
  }
  if (cleaned.length > maxChars) {
    return new KhalaUnsupportedRequestBadRequest({
      reason: `${field} must be ${maxChars} characters or fewer`,
    })
  }
  return rejectPrivateShapedValue(cleaned).pipe(Effect.as(cleaned))
}

const cleanPublicRef = (
  value: string,
  field: string,
): Effect.Effect<string, KhalaUnsupportedRequestBadRequest> => {
  const cleaned = value.trim()
  if (!isPublicSafeRef(cleaned)) {
    return new KhalaUnsupportedRequestBadRequest({
      reason: `${field} must be a bounded public-safe ref`,
    })
  }
  return Effect.succeed(cleaned)
}

const cleanOptionalPublicRef = (
  value: string | undefined,
  field: string,
): Effect.Effect<string | null, KhalaUnsupportedRequestBadRequest> => {
  const cleaned = cleanOptionalString(value)
  if (cleaned === null) {
    return Effect.succeed(null)
  }
  return cleanPublicRef(cleaned, field)
}

const cleanEvidenceRefs = (
  refs: ReadonlyArray<string> | undefined,
): Effect.Effect<ReadonlyArray<string>, KhalaUnsupportedRequestBadRequest> => {
  const uniqueRefs = [...new Set(refs ?? [])]
  if (uniqueRefs.length > KHALA_UNSUPPORTED_REQUEST_MAX_REFS) {
    return new KhalaUnsupportedRequestBadRequest({
      reason: `evidenceRefs must contain ${KHALA_UNSUPPORTED_REQUEST_MAX_REFS} refs or fewer`,
    })
  }
  return Effect.forEach(uniqueRefs, ref => cleanPublicRef(ref, 'evidenceRefs'))
}

const requestRef = (makeRequestRef?: () => string): string =>
  makeRequestRef?.() ?? `khala_unsupported:${compactRandomId('ur')}`

const issueRequiredFor = (
  triageKind: KhalaUnsupportedRequestTriageKind,
  githubIssueRef: string | null,
): boolean =>
  (triageKind === 'bug' || triageKind === 'missing_capability') &&
  githubIssueRef === null

const statusFor = (
  triageKind: KhalaUnsupportedRequestTriageKind,
  githubIssueRef: string | null,
  requestedStatus: KhalaUnsupportedRequestStatus | undefined,
): Effect.Effect<
  KhalaUnsupportedRequestStatus,
  KhalaUnsupportedRequestBadRequest
> => {
  if (requestedStatus === 'issue_opened' && githubIssueRef === null) {
    return new KhalaUnsupportedRequestBadRequest({
      reason: 'githubIssueRef is required when status is issue_opened',
    })
  }
  if (
    requestedStatus === 'needs_issue' &&
    !issueRequiredFor(triageKind, githubIssueRef)
  ) {
    return new KhalaUnsupportedRequestBadRequest({
      reason:
        'needs_issue status is only valid for bug or missing_capability triage',
    })
  }
  if (requestedStatus !== undefined) {
    return Effect.succeed(requestedStatus)
  }
  if (triageKind === 'wont_do') {
    return Effect.succeed('wont_do')
  }
  return Effect.succeed(
    issueRequiredFor(triageKind, githubIssueRef) ? 'needs_issue' : 'open',
  )
}

const nextActionFor = (
  record: Pick<
    KhalaUnsupportedRequestRecord,
    'forumTopicRef' | 'githubIssueRef' | 'status' | 'triageKind'
  >,
): KhalaUnsupportedRequestRecord['nextAction'] => {
  if (
    record.status === 'closed' ||
    record.status === 'wont_do' ||
    record.status === 'issue_opened'
  ) {
    return 'none'
  }
  if (issueRequiredFor(record.triageKind, record.githubIssueRef)) {
    return 'open_github_issue'
  }
  if (record.triageKind === 'needs_triage') {
    return 'triage'
  }
  if (
    record.status === 'open' &&
    record.forumTopicRef === null &&
    record.triageKind !== 'wont_do'
  ) {
    return 'link_forum_report'
  }
  return 'none'
}

const rowToRecord = (
  row: KhalaUnsupportedRequestRow,
): KhalaUnsupportedRequestRecord => {
  const evidenceRefs = parseJsonStringArray(row.evidence_refs_json)
  const base = {
    createdAt: row.created_at,
    evidenceRefs,
    forumTopicRef: row.forum_topic_ref,
    githubIssueRef: row.github_issue_ref,
    requestRef: row.request_ref,
    sourceKind: row.source_kind,
    sourceRef: row.source_ref,
    status: row.status,
    suggestedIssueTitle: row.suggested_issue_title,
    summary: row.summary,
    title: row.title,
    triageKind: row.triage_kind,
    updatedAt: row.updated_at,
  }
  const record = {
    ...base,
    issueRequired: issueRequiredFor(base.triageKind, base.githubIssueRef),
    nextAction: nextActionFor(base),
  }
  return record
}

export const makeD1KhalaUnsupportedRequestStore = (
  db: D1Database,
): KhalaUnsupportedRequestStore => ({
  upsert: async input => {
    await db
      .prepare(
        `INSERT INTO khala_unsupported_requests (
            request_ref,
            source_kind,
            source_ref,
            title,
            summary,
            triage_kind,
            status,
            forum_topic_ref,
            github_issue_ref,
            evidence_refs_json,
            suggested_issue_title,
            created_at,
            updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          ON CONFLICT(source_kind, source_ref) DO UPDATE SET
            title = excluded.title,
            summary = excluded.summary,
            triage_kind = excluded.triage_kind,
            status = excluded.status,
            forum_topic_ref = excluded.forum_topic_ref,
            github_issue_ref = excluded.github_issue_ref,
            evidence_refs_json = excluded.evidence_refs_json,
            suggested_issue_title = excluded.suggested_issue_title,
            updated_at = excluded.updated_at`,
      )
      .bind(
        input.requestRef,
        input.sourceKind,
        input.sourceRef,
        input.title,
        input.summary,
        input.triageKind,
        input.status,
        input.forumTopicRef,
        input.githubIssueRef,
        JSON.stringify(input.evidenceRefs),
        input.suggestedIssueTitle,
        input.createdAt,
        input.updatedAt,
      )
      .run()

    const row = await db
      .prepare(
        `SELECT request_ref,
                source_kind,
                source_ref,
                title,
                summary,
                triage_kind,
                status,
                forum_topic_ref,
                github_issue_ref,
                evidence_refs_json,
                suggested_issue_title,
                created_at,
                updated_at
           FROM khala_unsupported_requests
          WHERE source_kind = ? AND source_ref = ?`,
      )
      .bind(input.sourceKind, input.sourceRef)
      .first<KhalaUnsupportedRequestRow>()

    if (row === null) {
      throw new KhalaUnsupportedRequestStorageError({
        reason: 'unsupported request row was not readable after upsert',
      })
    }
    return rowToRecord(row)
  },

  listRecent: async input => {
    const limit = Math.min(
      Math.max(Math.trunc(input.limit), 1),
      KHALA_UNSUPPORTED_REQUEST_MAX_LIMIT,
    )
    const where: Array<string> = []
    const binds: Array<string | number> = []
    if (input.status !== undefined) {
      where.push('status = ?')
      binds.push(input.status)
    }
    if (input.triageKind !== undefined) {
      where.push('triage_kind = ?')
      binds.push(input.triageKind)
    }
    if (input.sourceKind !== undefined) {
      where.push('source_kind = ?')
      binds.push(input.sourceKind)
    }
    const rows = await db
      .prepare(
        `SELECT request_ref,
                source_kind,
                source_ref,
                title,
                summary,
                triage_kind,
                status,
                forum_topic_ref,
                github_issue_ref,
                evidence_refs_json,
                suggested_issue_title,
                created_at,
                updated_at
           FROM khala_unsupported_requests
          ${where.length === 0 ? '' : `WHERE ${where.join(' AND ')}`}
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .bind(...binds, limit)
      .all<KhalaUnsupportedRequestRow>()
    return rows.results.map(rowToRecord)
  },
})

const decodeCreateBody = (
  request: Request,
): Effect.Effect<
  KhalaUnsupportedRequestCreateBody,
  KhalaUnsupportedRequestBadRequest
> =>
  Effect.gen(function* () {
    const payload = yield* Effect.tryPromise({
      try: () => request.json(),
      catch: error =>
        new KhalaUnsupportedRequestBadRequest({
          reason: error instanceof Error ? error.message : 'invalid json',
        }),
    })

    return yield* S.decodeUnknownEffect(KhalaUnsupportedRequestCreateBody)(
      payload,
    ).pipe(
      Effect.mapError(
        error =>
          new KhalaUnsupportedRequestBadRequest({ reason: String(error) }),
      ),
    )
  })

const normalizeCreateBody = (
  body: KhalaUnsupportedRequestCreateBody,
  dependencies: OperatorKhalaUnsupportedRequestDependencies,
  now: string,
): Effect.Effect<
  KhalaUnsupportedRequestCreateInput,
  KhalaUnsupportedRequestBadRequest
> =>
  Effect.gen(function* () {
    const title = yield* cleanRequiredText(
      body.title,
      'title',
      KHALA_UNSUPPORTED_REQUEST_MAX_TITLE_CHARS,
    )
    const summary =
      (yield* cleanOptionalText(
        body.summary,
        'summary',
        KHALA_UNSUPPORTED_REQUEST_MAX_SUMMARY_CHARS,
      )) ?? ''
    const sourceKind = body.sourceKind ?? 'operator'
    const sourceRef = yield* cleanPublicRef(body.sourceRef, 'sourceRef')
    const forumTopicRef = yield* cleanOptionalPublicRef(
      body.forumTopicRef,
      'forumTopicRef',
    )
    const githubIssueRef = yield* cleanOptionalPublicRef(
      body.githubIssueRef,
      'githubIssueRef',
    )
    const evidenceRefs = yield* cleanEvidenceRefs(body.evidenceRefs)
    const triageKind = body.triageKind ?? 'needs_triage'
    const status = yield* statusFor(triageKind, githubIssueRef, body.status)
    const suggestedIssueTitle =
      (yield* cleanOptionalText(
        body.suggestedIssueTitle,
        'suggestedIssueTitle',
        KHALA_UNSUPPORTED_REQUEST_MAX_TITLE_CHARS,
      )) ?? `[Khala unsupported] ${title}`
    const candidate = {
      createdAt: now,
      evidenceRefs,
      forumTopicRef,
      githubIssueRef,
      requestRef:
        body.requestRef === undefined
          ? requestRef(dependencies.makeRequestRef)
          : yield* cleanPublicRef(body.requestRef, 'requestRef'),
      sourceKind,
      sourceRef,
      status,
      suggestedIssueTitle,
      summary,
      title,
      triageKind,
      updatedAt: now,
    }
    yield* rejectPrivateShapedValue(candidate)
    return candidate
  })

const parseEnumParam = <T extends string>(
  request: Request,
  key: string,
  allowed: ReadonlySet<T>,
): Effect.Effect<T | undefined, KhalaUnsupportedRequestBadRequest> => {
  const value = cleanOptionalString(
    new URL(request.url).searchParams.get(key) ?? undefined,
  )
  if (value === null) {
    return Effect.sync((): undefined => undefined)
  }
  if (allowed.has(value as T)) {
    return Effect.succeed(value as T)
  }
  return new KhalaUnsupportedRequestBadRequest({
    reason: `${key} is not supported`,
  })
}

const parseLimit = (request: Request): number => {
  const requestedLimit = Number(
    new URL(request.url).searchParams.get('limit') ?? '50',
  )
  return Number.isFinite(requestedLimit)
    ? Math.min(
        Math.max(Math.trunc(requestedLimit), 1),
        KHALA_UNSUPPORTED_REQUEST_MAX_LIMIT,
      )
    : 50
}

const unsupportedRequestEnvelope = (input: {
  generatedAt: string
  unsupportedRequests: ReadonlyArray<KhalaUnsupportedRequestRecord>
}) => ({
  cadence: {
    issueRef: 'OpenAgentsInc/openagents#6357',
    operatorCadence: 'each_khala_improvement_cycle',
    triageKinds: ['bug', 'missing_capability', 'wont_do', 'needs_triage'],
  },
  generatedAt: input.generatedAt,
  intake: {
    feedbackEndpoint: '/api/khala/feedback',
    forumFirstRef: 'https://openagents.com/forum/f/product-promises',
    operatorEndpoint: '/api/operator/khala/unsupported-requests',
    traceReviewEndpoint: '/api/operator/khala/trace-review',
  },
  schemaVersion: 'openagents.khala.unsupported_requests.v1' as const,
  unsupportedRequests: input.unsupportedRequests,
})

export const handleOperatorKhalaUnsupportedRequests = (
  request: Request,
  dependencies: OperatorKhalaUnsupportedRequestDependencies,
): Effect.Effect<HttpResponse> => {
  if (request.method !== 'GET' && request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['GET', 'POST']))
  }

  const nowIso = dependencies.nowIso ?? currentIsoTimestamp

  return Effect.gen(function* () {
    const authorized = yield* Effect.tryPromise({
      try: () => dependencies.requireAdminApiToken(request),
      catch: () => false,
    })
    if (!authorized) {
      return unauthorized()
    }

    if (request.method === 'POST') {
      const now = nowIso()
      const body = yield* decodeCreateBody(request)
      const input = yield* normalizeCreateBody(body, dependencies, now)
      const unsupportedRequest = yield* Effect.tryPromise({
        try: () => dependencies.store.upsert(input),
        catch: error =>
          new KhalaUnsupportedRequestStorageError({
            reason: error instanceof Error ? error.message : String(error),
          }),
      })
      return noStoreJsonResponse(
        {
          schemaVersion: 'openagents.khala.unsupported_requests.upsert.v1',
          unsupportedRequest,
        },
        { status: 201 },
      )
    }

    const status = yield* parseEnumParam(
      request,
      'status',
      new Set<KhalaUnsupportedRequestStatus>([
        'open',
        'needs_issue',
        'issue_opened',
        'closed',
        'wont_do',
      ]),
    )
    const triageKind = yield* parseEnumParam(
      request,
      'triageKind',
      new Set<KhalaUnsupportedRequestTriageKind>([
        'needs_triage',
        'bug',
        'missing_capability',
        'wont_do',
      ]),
    )
    const sourceKind = yield* parseEnumParam(
      request,
      'sourceKind',
      new Set<KhalaUnsupportedRequestSourceKind>([
        'trace_review',
        'khala_feedback',
        'forum',
        'operator',
      ]),
    )
    const unsupportedRequests = yield* Effect.tryPromise({
      try: () =>
        dependencies.store.listRecent({
          limit: parseLimit(request),
          sourceKind,
          status,
          triageKind,
        }),
      catch: error =>
        new KhalaUnsupportedRequestStorageError({
          reason: error instanceof Error ? error.message : String(error),
        }),
    })
    return noStoreJsonResponse(
      unsupportedRequestEnvelope({
        generatedAt: nowIso(),
        unsupportedRequests,
      }),
    )
  }).pipe(
    Effect.catchTags({
      KhalaUnsupportedRequestBadRequest: ({ reason }) =>
        Effect.succeed(
          noStoreJsonResponse(
            { error: 'bad_request', reason },
            { status: 400 },
          ),
        ),
      KhalaUnsupportedRequestStorageError: () =>
        Effect.succeed(
          noStoreJsonResponse(
            { error: 'khala_unsupported_requests_unavailable' },
            { status: 500 },
          ),
        ),
    }),
  )
}
