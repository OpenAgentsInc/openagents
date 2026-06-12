import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import type { AdjutantAssignment } from './adjutant-assignments'
import type { AdjutantResearchBrief } from './adjutant-research-briefs'
import { currentIsoTimestamp } from './runtime-primitives'

export const AdjutantTaskPacketFreshnessStatus = S.Literals([
  'missing',
  'current',
  'stale',
  'kept_current',
])
export type AdjutantTaskPacketFreshnessStatus =
  typeof AdjutantTaskPacketFreshnessStatus.Type

export type AdjutantTaskPacketFreshness = Readonly<{
  actorUserId: string | null
  assignmentId: string
  commitSha: string | null
  customerSafeSummary: string | null
  latestApprovedResearchBriefId: string | null
  operatorKeepReason: string | null
  researchBriefApprovedAt: string | null
  researchBriefId: string | null
  source: 'derived_missing' | 'recorded'
  sourceCardCount: number
  status: AdjutantTaskPacketFreshnessStatus
  taskSpecPath: string | null
  updatedAt: string
}>

type FreshnessRow = Readonly<{
  actor_user_id: string | null
  assignment_id: string
  commit_sha: string | null
  customer_safe_summary: string | null
  operator_keep_reason: string | null
  research_brief_approved_at: string | null
  research_brief_id: string | null
  source_card_count: number
  status: 'current' | 'stale' | 'kept_current'
  task_spec_path: string
  updated_at: string
}>

export type AdjutantTaskPacketFreshnessRuntime = Readonly<{
  nowIso: () => string
}>

export const systemAdjutantTaskPacketFreshnessRuntime: AdjutantTaskPacketFreshnessRuntime =
  {
    nowIso: currentIsoTimestamp,
  }

export class AdjutantTaskPacketFreshnessStorageError extends S.TaggedErrorClass<AdjutantTaskPacketFreshnessStorageError>()(
  'AdjutantTaskPacketFreshnessStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class AdjutantTaskPacketFreshnessUnsafePayload extends S.TaggedErrorClass<AdjutantTaskPacketFreshnessUnsafePayload>()(
  'AdjutantTaskPacketFreshnessUnsafePayload',
  {
    reason: S.String,
  },
) {}

export class AdjutantTaskPacketFreshnessValidationError extends S.TaggedErrorClass<AdjutantTaskPacketFreshnessValidationError>()(
  'AdjutantTaskPacketFreshnessValidationError',
  {
    reason: S.String,
  },
) {}

export type AdjutantTaskPacketFreshnessError =
  | AdjutantTaskPacketFreshnessStorageError
  | AdjutantTaskPacketFreshnessUnsafePayload
  | AdjutantTaskPacketFreshnessValidationError

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AdjutantTaskPacketFreshnessStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error =>
      new AdjutantTaskPacketFreshnessStorageError({ operation, error }),
  })

const nonEmptyBoundedText = (
  field: string,
  value: string,
  limit: number,
): Effect.Effect<string, AdjutantTaskPacketFreshnessValidationError> => {
  const text = value.trim()

  if (text === '') {
    return Effect.fail(
      new AdjutantTaskPacketFreshnessValidationError({
        reason: `${field} is required.`,
      }),
    )
  }

  if (text.length > limit) {
    return Effect.fail(
      new AdjutantTaskPacketFreshnessValidationError({
        reason: `${field} must be ${limit} characters or fewer.`,
      }),
    )
  }

  return Effect.succeed(text)
}

const assertSafe = (
  value: unknown,
): Effect.Effect<void, AdjutantTaskPacketFreshnessUnsafePayload> =>
  containsProviderSecretMaterial(JSON.stringify(value))
    ? Effect.fail(
        new AdjutantTaskPacketFreshnessUnsafePayload({
          reason: 'Task packet freshness payload contains secret-shaped material.',
        }),
      )
    : Effect.void

const freshnessFromRow = (
  assignment: AdjutantAssignment,
  latestApprovedBrief: AdjutantResearchBrief | null,
  row: FreshnessRow | null,
): AdjutantTaskPacketFreshness => {
  if (assignment.taskSpecPath === null) {
    return {
      actorUserId: null,
      assignmentId: assignment.id,
      commitSha: assignment.commitSha,
      customerSafeSummary: null,
      latestApprovedResearchBriefId: latestApprovedBrief?.id ?? null,
      operatorKeepReason: null,
      researchBriefApprovedAt: null,
      researchBriefId: null,
      source: 'derived_missing',
      sourceCardCount: 0,
      status: 'missing',
      taskSpecPath: null,
      updatedAt: assignment.updatedAt,
    }
  }

  if (row === null) {
    return {
      actorUserId: null,
      assignmentId: assignment.id,
      commitSha: assignment.commitSha,
      customerSafeSummary: null,
      latestApprovedResearchBriefId: latestApprovedBrief?.id ?? null,
      operatorKeepReason: null,
      researchBriefApprovedAt: null,
      researchBriefId: null,
      source: 'derived_missing',
      sourceCardCount: 0,
      status: latestApprovedBrief === null ? 'current' : 'stale',
      taskSpecPath: assignment.taskSpecPath,
      updatedAt: assignment.updatedAt,
    }
  }

  const staleByBrief =
    latestApprovedBrief !== null &&
    row.research_brief_id !== latestApprovedBrief.id &&
    row.status !== 'kept_current'

  return {
    actorUserId: row.actor_user_id,
    assignmentId: row.assignment_id,
    commitSha: row.commit_sha,
    customerSafeSummary: row.customer_safe_summary,
    latestApprovedResearchBriefId: latestApprovedBrief?.id ?? null,
    operatorKeepReason: row.operator_keep_reason,
    researchBriefApprovedAt: row.research_brief_approved_at,
    researchBriefId: row.research_brief_id,
    source: 'recorded',
    sourceCardCount: row.source_card_count,
    status: staleByBrief ? 'stale' : row.status,
    taskSpecPath: row.task_spec_path,
    updatedAt: row.updated_at,
  }
}

const readRow = (
  db: D1Database,
  assignmentId: string,
): Effect.Effect<FreshnessRow | null, AdjutantTaskPacketFreshnessStorageError> =>
  d1Effect('adjutantTaskPacketFreshness.read', () =>
    db
      .prepare(
        `SELECT assignment_id,
                task_spec_path,
                commit_sha,
                status,
                research_brief_id,
                research_brief_approved_at,
                source_card_count,
                operator_keep_reason,
                customer_safe_summary,
                actor_user_id,
                updated_at
           FROM adjutant_task_packet_freshness
          WHERE assignment_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(assignmentId)
      .first<FreshnessRow>(),
  )

const readFreshness = (
  db: D1Database,
  assignment: AdjutantAssignment,
  latestApprovedBrief: AdjutantResearchBrief | null,
): Effect.Effect<
  AdjutantTaskPacketFreshness,
  AdjutantTaskPacketFreshnessError
> =>
  readRow(db, assignment.id).pipe(
    Effect.map(row => freshnessFromRow(assignment, latestApprovedBrief, row)),
  )

const recordGenerated = (
  db: D1Database,
  runtime: AdjutantTaskPacketFreshnessRuntime,
  input: Readonly<{
    assignment: AdjutantAssignment
    researchBrief: AdjutantResearchBrief | null
  }>,
): Effect.Effect<AdjutantTaskPacketFreshness, AdjutantTaskPacketFreshnessError> =>
  Effect.gen(function* () {
    if (input.assignment.taskSpecPath === null) {
      return yield* new AdjutantTaskPacketFreshnessValidationError({
        reason: 'Generated task packet requires a task spec path.',
      })
    }

    yield* assertSafe({
      assignmentId: input.assignment.id,
      researchBriefId: input.researchBrief?.id ?? null,
      taskSpecPath: input.assignment.taskSpecPath,
    })

    const now = runtime.nowIso()

    yield* d1Effect('adjutantTaskPacketFreshness.generated.upsert', () =>
      db
        .prepare(
          `INSERT INTO adjutant_task_packet_freshness
             (assignment_id,
              task_spec_path,
              commit_sha,
              status,
              research_brief_id,
              research_brief_approved_at,
              source_card_count,
              created_at,
              updated_at)
           VALUES (?, ?, ?, 'current', ?, ?, ?, ?, ?)
           ON CONFLICT(assignment_id) DO UPDATE SET
             task_spec_path = excluded.task_spec_path,
             commit_sha = excluded.commit_sha,
             status = 'current',
             research_brief_id = excluded.research_brief_id,
             research_brief_approved_at = excluded.research_brief_approved_at,
             source_card_count = excluded.source_card_count,
             operator_keep_reason = NULL,
             customer_safe_summary = NULL,
             actor_user_id = NULL,
             stale_at = NULL,
             kept_at = NULL,
             updated_at = excluded.updated_at,
             archived_at = NULL`,
        )
        .bind(
          input.assignment.id,
          input.assignment.taskSpecPath,
          input.assignment.commitSha,
          input.researchBrief?.id ?? null,
          input.researchBrief?.approvedAt ?? null,
          input.researchBrief?.sourceCards.length ?? 0,
          now,
          now,
        )
        .run(),
    )

    return yield* readFreshness(db, input.assignment, input.researchBrief)
  })

const markStaleForApprovedResearch = (
  db: D1Database,
  runtime: AdjutantTaskPacketFreshnessRuntime,
  input: Readonly<{
    assignment: AdjutantAssignment
    researchBrief: AdjutantResearchBrief
  }>,
): Effect.Effect<AdjutantTaskPacketFreshness, AdjutantTaskPacketFreshnessError> =>
  Effect.gen(function* () {
    if (input.assignment.taskSpecPath === null) {
      return yield* readFreshness(db, input.assignment, input.researchBrief)
    }

    const current = yield* readFreshness(db, input.assignment, input.researchBrief)

    if (
      current.researchBriefId === input.researchBrief.id ||
      current.status === 'kept_current'
    ) {
      return current
    }

    const now = runtime.nowIso()

    yield* d1Effect('adjutantTaskPacketFreshness.stale.upsert', () =>
      db
        .prepare(
          `INSERT INTO adjutant_task_packet_freshness
             (assignment_id,
              task_spec_path,
              commit_sha,
              status,
              research_brief_id,
              research_brief_approved_at,
              source_card_count,
              stale_at,
              created_at,
              updated_at)
           VALUES (?, ?, ?, 'stale', ?, ?, ?, ?, ?, ?)
           ON CONFLICT(assignment_id) DO UPDATE SET
             status = 'stale',
             research_brief_id = excluded.research_brief_id,
             research_brief_approved_at = excluded.research_brief_approved_at,
             source_card_count = excluded.source_card_count,
             stale_at = excluded.stale_at,
             updated_at = excluded.updated_at,
             archived_at = NULL`,
        )
        .bind(
          input.assignment.id,
          input.assignment.taskSpecPath,
          input.assignment.commitSha,
          input.researchBrief.id,
          input.researchBrief.approvedAt,
          input.researchBrief.sourceCards.length,
          now,
          now,
          now,
        )
        .run(),
    )

    return yield* readFreshness(db, input.assignment, input.researchBrief)
  })

const keepCurrent = (
  db: D1Database,
  runtime: AdjutantTaskPacketFreshnessRuntime,
  input: Readonly<{
    actorUserId: string
    assignment: AdjutantAssignment
    customerSafeSummary: string
    latestApprovedBrief: AdjutantResearchBrief | null
    reason: string
  }>,
): Effect.Effect<AdjutantTaskPacketFreshness, AdjutantTaskPacketFreshnessError> =>
  Effect.gen(function* () {
    if (input.assignment.taskSpecPath === null) {
      return yield* new AdjutantTaskPacketFreshnessValidationError({
        reason: 'Cannot keep a missing task packet current.',
      })
    }

    const reason = yield* nonEmptyBoundedText('reason', input.reason, 1000)
    const customerSafeSummary = yield* nonEmptyBoundedText(
      'customerSafeSummary',
      input.customerSafeSummary,
      500,
    )
    yield* assertSafe({ customerSafeSummary, reason })

    const now = runtime.nowIso()

    yield* d1Effect('adjutantTaskPacketFreshness.keep.upsert', () =>
      db
        .prepare(
          `INSERT INTO adjutant_task_packet_freshness
             (assignment_id,
              task_spec_path,
              commit_sha,
              status,
              research_brief_id,
              research_brief_approved_at,
              source_card_count,
              operator_keep_reason,
              customer_safe_summary,
              actor_user_id,
              kept_at,
              created_at,
              updated_at)
           VALUES (?, ?, ?, 'kept_current', ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(assignment_id) DO UPDATE SET
             status = 'kept_current',
             research_brief_id = excluded.research_brief_id,
             research_brief_approved_at = excluded.research_brief_approved_at,
             source_card_count = excluded.source_card_count,
             operator_keep_reason = excluded.operator_keep_reason,
             customer_safe_summary = excluded.customer_safe_summary,
             actor_user_id = excluded.actor_user_id,
             kept_at = excluded.kept_at,
             updated_at = excluded.updated_at,
             archived_at = NULL`,
        )
        .bind(
          input.assignment.id,
          input.assignment.taskSpecPath,
          input.assignment.commitSha,
          input.latestApprovedBrief?.id ?? null,
          input.latestApprovedBrief?.approvedAt ?? null,
          input.latestApprovedBrief?.sourceCards.length ?? 0,
          reason,
          customerSafeSummary,
          input.actorUserId,
          now,
          now,
          now,
        )
        .run(),
    )

    return yield* readFreshness(db, input.assignment, input.latestApprovedBrief)
  })

export const makeAdjutantTaskPacketFreshnessService = (
  db: D1Database,
  runtime: AdjutantTaskPacketFreshnessRuntime =
    systemAdjutantTaskPacketFreshnessRuntime,
) => ({
  keepCurrent: Effect.fn('AdjutantTaskPacketFreshness.keepCurrent')(
    (input: Parameters<typeof keepCurrent>[2]) =>
      keepCurrent(db, runtime, input),
  ),
  markStaleForApprovedResearch: Effect.fn(
    'AdjutantTaskPacketFreshness.markStaleForApprovedResearch',
  )((input: Parameters<typeof markStaleForApprovedResearch>[2]) =>
    markStaleForApprovedResearch(db, runtime, input),
  ),
  readFreshness: Effect.fn('AdjutantTaskPacketFreshness.readFreshness')(
    (assignment: AdjutantAssignment, latestApprovedBrief: AdjutantResearchBrief | null) =>
      readFreshness(db, assignment, latestApprovedBrief),
  ),
  recordGenerated: Effect.fn('AdjutantTaskPacketFreshness.recordGenerated')(
    (input: Parameters<typeof recordGenerated>[2]) =>
      recordGenerated(db, runtime, input),
  ),
})
