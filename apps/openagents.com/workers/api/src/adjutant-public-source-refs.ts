import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import {
  ExplicitPublicSourceRef as ExplicitPublicSourceRefSchema,
  PublicSourceRefKind as PublicSourceRefKindSchema,
  PublicSourceRefStatus as PublicSourceRefStatusSchema,
  type ExplicitPublicSourceRef as ExplicitPublicSourceRefShape,
  type PublicSourceRefKind as PublicSourceRefKindShape,
  type PublicSourceRefStatus as PublicSourceRefStatusShape,
} from './adjutant-enrichment-planner'
import { openAgentsDatabase } from './runtime'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

type AdjutantPublicSourceRefEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

export type AdjutantPublicSourceRefRuntime = Readonly<{
  makeSourceRefId: () => string
  nowIso: () => string
}>

export const systemAdjutantPublicSourceRefRuntime: AdjutantPublicSourceRefRuntime =
  {
    makeSourceRefId: () => compactRandomId('adjutant_public_source_ref'),
    nowIso: currentIsoTimestamp,
  }

const MAX_LABEL_CHARS = 240
const MAX_REASON_CHARS = 500
const MAX_URL_CHARS = 2048

export const AdjutantPublicSourceRef = S.Struct({
  id: S.String,
  assignmentId: S.String,
  softwareOrderId: S.NullOr(S.String),
  siteId: S.NullOr(S.String),
  kind: PublicSourceRefKindSchema,
  status: PublicSourceRefStatusSchema,
  url: S.String,
  normalizedDomain: S.String,
  label: S.NullOr(S.String),
  publicSafe: S.Boolean,
  proposedByUserId: S.NullOr(S.String),
  reviewedByUserId: S.NullOr(S.String),
  reviewReason: S.NullOr(S.String),
  approvedAt: S.NullOr(S.String),
  rejectedAt: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
  archivedAt: S.NullOr(S.String),
})
export type AdjutantPublicSourceRef =
  typeof AdjutantPublicSourceRef.Type

export const CreateAdjutantPublicSourceRefInput = S.Struct({
  assignmentId: S.String,
  kind: PublicSourceRefKindSchema,
  label: S.optionalKey(S.NullOr(S.String)),
  proposedByUserId: S.optionalKey(S.NullOr(S.String)),
  repositoryPrivate: S.optionalKey(S.Boolean),
  siteId: S.optionalKey(S.NullOr(S.String)),
  softwareOrderId: S.optionalKey(S.NullOr(S.String)),
  status: S.optionalKey(PublicSourceRefStatusSchema),
  url: S.String,
})
export type CreateAdjutantPublicSourceRefInput =
  typeof CreateAdjutantPublicSourceRefInput.Type

export const ReviewAdjutantPublicSourceRefInput = S.Struct({
  publicSafe: S.optionalKey(S.Boolean),
  reviewReason: S.optionalKey(S.NullOr(S.String)),
  reviewedByUserId: S.optionalKey(S.NullOr(S.String)),
  sourceRefId: S.String,
  status: PublicSourceRefStatusSchema,
})
export type ReviewAdjutantPublicSourceRefInput =
  typeof ReviewAdjutantPublicSourceRefInput.Type

type AdjutantPublicSourceRefRow = Readonly<{
  approved_at: string | null
  archived_at: string | null
  assignment_id: string
  created_at: string
  id: string
  kind: PublicSourceRefKindShape
  label: string | null
  normalized_domain: string
  proposed_by_user_id: string | null
  public_safe: number
  rejected_at: string | null
  review_reason: string | null
  reviewed_by_user_id: string | null
  site_id: string | null
  software_order_id: string | null
  status: PublicSourceRefStatusShape
  updated_at: string
  url: string
}>

export class AdjutantPublicSourceRefStorageError extends S.TaggedErrorClass<AdjutantPublicSourceRefStorageError>()(
  'AdjutantPublicSourceRefStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class AdjutantPublicSourceRefUnsafePayload extends S.TaggedErrorClass<AdjutantPublicSourceRefUnsafePayload>()(
  'AdjutantPublicSourceRefUnsafePayload',
  {
    reason: S.String,
  },
) {}

export class AdjutantPublicSourceRefValidationError extends S.TaggedErrorClass<AdjutantPublicSourceRefValidationError>()(
  'AdjutantPublicSourceRefValidationError',
  {
    reason: S.String,
  },
) {}

export type AdjutantPublicSourceRefError =
  | AdjutantPublicSourceRefStorageError
  | AdjutantPublicSourceRefUnsafePayload
  | AdjutantPublicSourceRefValidationError

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AdjutantPublicSourceRefStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error =>
      new AdjutantPublicSourceRefStorageError({ operation, error }),
  })

const sourceRefFromRow = (
  row: AdjutantPublicSourceRefRow,
): AdjutantPublicSourceRef => ({
  id: row.id,
  assignmentId: row.assignment_id,
  softwareOrderId: row.software_order_id,
  siteId: row.site_id,
  kind: row.kind,
  status: row.status,
  url: row.url,
  normalizedDomain: row.normalized_domain,
  label: row.label,
  publicSafe: row.public_safe === 1,
  proposedByUserId: row.proposed_by_user_id,
  reviewedByUserId: row.reviewed_by_user_id,
  reviewReason: row.review_reason,
  approvedAt: row.approved_at,
  rejectedAt: row.rejected_at,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  archivedAt: row.archived_at,
})

const nullableText = (value: string | null | undefined): string | null => {
  const text = value?.trim().replace(/\s+/g, ' ')

  return text === undefined || text === '' ? null : text
}

const boundedOptionalText = (
  field: string,
  value: string | null | undefined,
  maxCharacters: number,
): Effect.Effect<
  string | null,
  | AdjutantPublicSourceRefUnsafePayload
  | AdjutantPublicSourceRefValidationError
> => {
  const text = nullableText(value)

  if (text === null) {
    return Effect.succeed(null)
  }

  if (text.length > maxCharacters) {
    return Effect.fail(
      new AdjutantPublicSourceRefValidationError({
        reason: `${field} exceeds ${maxCharacters} characters.`,
      }),
    )
  }

  if (containsProviderSecretMaterial(text)) {
    return Effect.fail(
      new AdjutantPublicSourceRefUnsafePayload({
        reason: `${field} contains secret-shaped material.`,
      }),
    )
  }

  return Effect.succeed(text)
}

const blockedHostPatterns: ReadonlyArray<RegExp> = [
  /^localhost$/iu,
  /^127\./u,
  /^10\./u,
  /^192\.168\./u,
  /^172\.(1[6-9]|2[0-9]|3[0-1])\./u,
  /^0\.0\.0\.0$/u,
  /^\[?::1\]?$/u,
  /\.local$/iu,
  /\.internal$/iu,
]

const parsePublicUrl = (
  value: string,
): Effect.Effect<URL, AdjutantPublicSourceRefValidationError> =>
  Effect.try({
    catch: error =>
      new AdjutantPublicSourceRefValidationError({
        reason: error instanceof Error ? error.message : 'Invalid source URL.',
      }),
    try: () => new URL(value),
  }).pipe(
    Effect.flatMap(url => {
      if (url.protocol !== 'https:' && url.protocol !== 'http:') {
        return Effect.fail(
          new AdjutantPublicSourceRefValidationError({
            reason: 'Public source refs must use http or https URLs.',
          }),
        )
      }

      if (
        blockedHostPatterns.some(pattern => pattern.test(url.hostname)) ||
        url.username !== '' ||
        url.password !== ''
      ) {
        return Effect.fail(
          new AdjutantPublicSourceRefValidationError({
            reason: 'Public source ref URL is not an allowed public URL.',
          }),
        )
      }

      return Effect.succeed(url)
    }),
  )

const assertKindMatchesUrl = (
  kind: PublicSourceRefKindShape,
  url: URL,
  repositoryPrivate: boolean,
): Effect.Effect<void, AdjutantPublicSourceRefValidationError> => {
  const hostname = url.hostname.toLowerCase()
  const pathParts = url.pathname.split('/').filter(part => part !== '')

  if (kind === 'github_repository') {
    if (repositoryPrivate) {
      return Effect.fail(
        new AdjutantPublicSourceRefValidationError({
          reason: 'Private GitHub repositories cannot be Exa public source refs.',
        }),
      )
    }

    return hostname === 'github.com' && pathParts.length >= 2
      ? Effect.void
      : Effect.fail(
          new AdjutantPublicSourceRefValidationError({
            reason:
              'GitHub repository source refs must be public github.com owner/repo URLs.',
          }),
        )
  }

  if (kind === 'github_profile') {
    return hostname === 'github.com' && pathParts.length === 1
      ? Effect.void
      : Effect.fail(
          new AdjutantPublicSourceRefValidationError({
            reason:
              'GitHub profile source refs must be public github.com profile URLs.',
          }),
        )
  }

  if (kind === 'linkedin_profile') {
    return hostname.endsWith('linkedin.com') && pathParts[0] === 'in'
      ? Effect.void
      : Effect.fail(
          new AdjutantPublicSourceRefValidationError({
            reason: 'LinkedIn profile source refs must use linkedin.com/in URLs.',
          }),
        )
  }

  if (kind === 'x_profile') {
    return (hostname === 'x.com' || hostname === 'twitter.com') &&
      pathParts.length >= 1
      ? Effect.void
      : Effect.fail(
          new AdjutantPublicSourceRefValidationError({
            reason: 'X/Twitter profile source refs must use x.com or twitter.com URLs.',
          }),
        )
  }

  return Effect.void
}

const assertSafeInput = (
  input: unknown,
): Effect.Effect<void, AdjutantPublicSourceRefUnsafePayload> =>
  containsProviderSecretMaterial(JSON.stringify(input))
    ? Effect.fail(
        new AdjutantPublicSourceRefUnsafePayload({
          reason: 'Public source ref contains secret-shaped material.',
        }),
      )
    : Effect.void

const normalizedUrl = (url: URL): string => {
  url.hash = ''

  return url.toString()
}

const createSourceRef = (
  db: D1Database,
  runtime: AdjutantPublicSourceRefRuntime,
  input: CreateAdjutantPublicSourceRefInput,
): Effect.Effect<AdjutantPublicSourceRef, AdjutantPublicSourceRefError> =>
  Effect.gen(function* () {
    yield* assertSafeInput(input)

    if (input.url.length > MAX_URL_CHARS) {
      return yield* new AdjutantPublicSourceRefValidationError({
        reason: `url exceeds ${MAX_URL_CHARS} characters.`,
      })
    }

    const url = yield* parsePublicUrl(input.url)
    yield* assertKindMatchesUrl(
      input.kind,
      url,
      input.repositoryPrivate === true,
    )

    const label = yield* boundedOptionalText(
      'label',
      input.label,
      MAX_LABEL_CHARS,
    )
    const now = runtime.nowIso()
    const status = input.status ?? 'proposed'
    const publicSafe = status === 'public_safe'
    const approvedAt =
      status === 'approved' || status === 'public_safe' ? now : null
    const rejectedAt = status === 'rejected' ? now : null
    const sourceRefId = runtime.makeSourceRefId()
    const cleanUrl = normalizedUrl(url)

    yield* d1Effect('adjutantPublicSourceRefs.insert', () =>
      db
        .prepare(
          `INSERT INTO adjutant_public_source_refs
             (id,
              assignment_id,
              software_order_id,
              site_id,
              kind,
              status,
              url,
              normalized_domain,
              label,
              public_safe,
              proposed_by_user_id,
              approved_at,
              rejected_at,
              created_at,
              updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          sourceRefId,
          input.assignmentId,
          input.softwareOrderId ?? null,
          input.siteId ?? null,
          input.kind,
          status,
          cleanUrl,
          url.hostname,
          label,
          publicSafe ? 1 : 0,
          input.proposedByUserId ?? null,
          approvedAt,
          rejectedAt,
          now,
          now,
        )
        .run(),
    )

    return {
      id: sourceRefId,
      assignmentId: input.assignmentId,
      softwareOrderId: input.softwareOrderId ?? null,
      siteId: input.siteId ?? null,
      kind: input.kind,
      status,
      url: cleanUrl,
      normalizedDomain: url.hostname,
      label,
      publicSafe,
      proposedByUserId: input.proposedByUserId ?? null,
      reviewedByUserId: null,
      reviewReason: null,
      approvedAt,
      rejectedAt,
      createdAt: now,
      updatedAt: now,
      archivedAt: null,
    }
  })

const reviewSourceRef = (
  db: D1Database,
  runtime: AdjutantPublicSourceRefRuntime,
  input: ReviewAdjutantPublicSourceRefInput,
): Effect.Effect<void, AdjutantPublicSourceRefError> =>
  Effect.gen(function* () {
    const reason = yield* boundedOptionalText(
      'reviewReason',
      input.reviewReason,
      MAX_REASON_CHARS,
    )
    const now = runtime.nowIso()
    const publicSafe =
      input.publicSafe === true || input.status === 'public_safe'

    yield* d1Effect('adjutantPublicSourceRefs.review', () =>
      db
        .prepare(
          `UPDATE adjutant_public_source_refs
              SET status = ?,
                  public_safe = ?,
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
          publicSafe ? 1 : 0,
          input.reviewedByUserId ?? null,
          reason,
          input.status === 'approved' || input.status === 'public_safe'
            ? now
            : null,
          input.status === 'rejected' ? now : null,
          now,
          input.sourceRefId,
        )
        .run(),
    )
  })

const listForAssignment = (
  db: D1Database,
  assignmentId: string,
): Effect.Effect<
  ReadonlyArray<AdjutantPublicSourceRef>,
  AdjutantPublicSourceRefStorageError
> =>
  d1Effect('adjutantPublicSourceRefs.listForAssignment', () =>
    db
      .prepare(
        `SELECT id,
                assignment_id,
                software_order_id,
                site_id,
                kind,
                status,
                url,
                normalized_domain,
                label,
                public_safe,
                proposed_by_user_id,
                reviewed_by_user_id,
                review_reason,
                approved_at,
                rejected_at,
                created_at,
                updated_at,
                archived_at
           FROM adjutant_public_source_refs
          WHERE assignment_id = ?
            AND archived_at IS NULL
          ORDER BY created_at DESC`,
      )
      .bind(assignmentId)
      .all<AdjutantPublicSourceRefRow>(),
  ).pipe(Effect.map(result => result.results.map(sourceRefFromRow)))

export const plannerSourceRefs = (
  sourceRefs: ReadonlyArray<AdjutantPublicSourceRef>,
): ReadonlyArray<ExplicitPublicSourceRefShape> =>
  sourceRefs
    .filter(
      sourceRef =>
        sourceRef.status === 'approved' || sourceRef.status === 'public_safe',
    )
    .map(sourceRef =>
      ExplicitPublicSourceRefSchema.make({
        id: sourceRef.id,
        kind: sourceRef.kind,
        status: sourceRef.status,
        url: sourceRef.url,
        ...(sourceRef.label === null ? {} : { label: sourceRef.label }),
      }),
    )

const plannerSourceRefsForAssignment = (
  db: D1Database,
  assignmentId: string,
): Effect.Effect<
  ReadonlyArray<ExplicitPublicSourceRefShape>,
  AdjutantPublicSourceRefStorageError
> => listForAssignment(db, assignmentId).pipe(Effect.map(plannerSourceRefs))

export const makeAdjutantPublicSourceRefService = (
  db: D1Database,
  runtime: AdjutantPublicSourceRefRuntime =
    systemAdjutantPublicSourceRefRuntime,
) => ({
  createSourceRef: Effect.fn('AdjutantPublicSourceRefService.createSourceRef')(
    (input: CreateAdjutantPublicSourceRefInput) =>
      createSourceRef(db, runtime, input),
  ),
  listForAssignment: Effect.fn(
    'AdjutantPublicSourceRefService.listForAssignment',
  )((assignmentId: string) => listForAssignment(db, assignmentId)),
  plannerSourceRefsForAssignment: Effect.fn(
    'AdjutantPublicSourceRefService.plannerSourceRefsForAssignment',
  )((assignmentId: string) => plannerSourceRefsForAssignment(db, assignmentId)),
  reviewSourceRef: Effect.fn(
    'AdjutantPublicSourceRefService.reviewSourceRef',
  )((input: ReviewAdjutantPublicSourceRefInput) =>
    reviewSourceRef(db, runtime, input),
  ),
})

export class AdjutantPublicSourceRefService extends Context.Service<
  AdjutantPublicSourceRefService,
  ReturnType<typeof makeAdjutantPublicSourceRefService>
>()('@openagentsinc/autopilot-omega/AdjutantPublicSourceRefService') {
  static layer = (
    env: AdjutantPublicSourceRefEnv,
    runtime: AdjutantPublicSourceRefRuntime =
      systemAdjutantPublicSourceRefRuntime,
  ) =>
    Layer.succeed(
      AdjutantPublicSourceRefService,
      makeAdjutantPublicSourceRefService(openAgentsDatabase(env), runtime),
    )
}
