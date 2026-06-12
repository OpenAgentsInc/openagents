import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export type AdjutantAdjustmentRuntime = Readonly<{
  makeAdjustmentId: () => string
  nowIso: () => string
}>

export const systemAdjutantAdjustmentRuntime: AdjutantAdjustmentRuntime = {
  makeAdjustmentId: () => compactRandomId('adjutant_adjustment'),
  nowIso: currentIsoTimestamp,
}

export const AdjutantAdjustmentStatus = S.Literals([
  'requested',
  'queued',
  'running',
  'review_needed',
  'completed',
  'rejected',
  'canceled',
  'failed',
])
export type AdjutantAdjustmentStatus = typeof AdjutantAdjustmentStatus.Type

export const AdjutantAdjustmentContinuationMode = S.Literals([
  'follow_up_turn',
  'new_goal_run',
])
export type AdjutantAdjustmentContinuationMode =
  typeof AdjutantAdjustmentContinuationMode.Type

export const AdjutantAdjustmentRequest = S.Struct({
  id: S.String,
  assignmentId: S.String,
  softwareOrderId: S.NullOr(S.String),
  siteId: S.String,
  goalId: S.NullOr(S.String),
  requestedByUserId: S.NullOr(S.String),
  instruction: S.String,
  status: AdjutantAdjustmentStatus,
  continuationMode: S.NullOr(AdjutantAdjustmentContinuationMode),
  sourceRunId: S.NullOr(S.String),
  continuationRunId: S.NullOr(S.String),
  resultingVersionId: S.NullOr(S.String),
  visibility: S.Literals(['private', 'team', 'public']),
  createdAt: S.String,
  updatedAt: S.String,
  completedAt: S.NullOr(S.String),
  archivedAt: S.NullOr(S.String),
})
export type AdjutantAdjustmentRequest = typeof AdjutantAdjustmentRequest.Type

export const CreateAdjutantAdjustmentInput = S.Struct({
  assignmentId: S.String,
  siteId: S.String,
  instruction: S.String,
  goalId: S.optionalKey(S.NullOr(S.String)),
  requestedByUserId: S.optionalKey(S.String),
  softwareOrderId: S.optionalKey(S.NullOr(S.String)),
  sourceRunId: S.optionalKey(S.NullOr(S.String)),
  visibility: S.optionalKey(S.Literals(['private', 'team', 'public'])),
})
export type CreateAdjutantAdjustmentInput =
  typeof CreateAdjutantAdjustmentInput.Type

export const UpdateAdjutantAdjustmentInput = S.Struct({
  adjustmentId: S.String,
  continuationMode: S.optionalKey(S.NullOr(AdjutantAdjustmentContinuationMode)),
  continuationRunId: S.optionalKey(S.NullOr(S.String)),
  resultingVersionId: S.optionalKey(S.NullOr(S.String)),
  status: S.optionalKey(AdjutantAdjustmentStatus),
})
export type UpdateAdjutantAdjustmentInput =
  typeof UpdateAdjutantAdjustmentInput.Type

type AdjutantAdjustmentRow = Readonly<{
  archived_at: string | null
  assignment_id: string
  completed_at: string | null
  continuation_mode: AdjutantAdjustmentContinuationMode | null
  continuation_run_id: string | null
  created_at: string
  goal_id: string | null
  id: string
  instruction: string
  requested_by_user_id: string | null
  resulting_version_id: string | null
  site_id: string
  software_order_id: string | null
  source_run_id: string | null
  status: AdjutantAdjustmentStatus
  updated_at: string
  visibility: 'private' | 'team' | 'public'
}>

export class AdjutantAdjustmentNotFound extends S.TaggedErrorClass<AdjutantAdjustmentNotFound>()(
  'AdjutantAdjustmentNotFound',
  {
    adjustmentId: S.String,
  },
) {}

export class AdjutantAdjustmentStorageError extends S.TaggedErrorClass<AdjutantAdjustmentStorageError>()(
  'AdjutantAdjustmentStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class AdjutantAdjustmentUnsafePayload extends S.TaggedErrorClass<AdjutantAdjustmentUnsafePayload>()(
  'AdjutantAdjustmentUnsafePayload',
  {
    reason: S.String,
  },
) {}

export class AdjutantAdjustmentValidationError extends S.TaggedErrorClass<AdjutantAdjustmentValidationError>()(
  'AdjutantAdjustmentValidationError',
  {
    reason: S.String,
  },
) {}

export type AdjutantAdjustmentError =
  | AdjutantAdjustmentNotFound
  | AdjutantAdjustmentStorageError
  | AdjutantAdjustmentUnsafePayload
  | AdjutantAdjustmentValidationError

const adjustmentSelectColumns = `id,
       assignment_id,
       software_order_id,
       site_id,
       goal_id,
       requested_by_user_id,
       instruction,
       status,
       continuation_mode,
       source_run_id,
       continuation_run_id,
       resulting_version_id,
       visibility,
       created_at,
       updated_at,
       completed_at,
       archived_at`

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AdjutantAdjustmentStorageError> =>
  Effect.tryPromise({
    catch: error => new AdjutantAdjustmentStorageError({ operation, error }),
    try: run,
  })

const adjustmentFromRow = (
  row: AdjutantAdjustmentRow,
): AdjutantAdjustmentRequest => ({
  id: row.id,
  assignmentId: row.assignment_id,
  softwareOrderId: row.software_order_id,
  siteId: row.site_id,
  goalId: row.goal_id,
  requestedByUserId: row.requested_by_user_id,
  instruction: row.instruction,
  status: row.status,
  continuationMode: row.continuation_mode,
  sourceRunId: row.source_run_id,
  continuationRunId: row.continuation_run_id,
  resultingVersionId: row.resulting_version_id,
  visibility: row.visibility,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  completedAt: row.completed_at,
  archivedAt: row.archived_at,
})

const nullableInput = (value: string | null | undefined): string | null => {
  const text = typeof value === 'string' ? value.trim() : ''

  return text === '' ? null : text
}

const assertInstructionSafe = (
  instruction: string,
): Effect.Effect<
  string,
  AdjutantAdjustmentUnsafePayload | AdjutantAdjustmentValidationError
> =>
  Effect.gen(function* () {
    const text = instruction.trim()

    if (text === '') {
      return yield* new AdjutantAdjustmentValidationError({
        reason: 'Adjustment instruction is required.',
      })
    }

    if (text.length > 4000) {
      return yield* new AdjutantAdjustmentValidationError({
        reason: 'Adjustment instruction must be 4000 characters or fewer.',
      })
    }

    if (containsProviderSecretMaterial(text)) {
      return yield* new AdjutantAdjustmentUnsafePayload({
        reason: 'Adjustment instruction contains secret-shaped material.',
      })
    }

    return text
  })

const readAdjustmentById = (
  db: D1Database,
  adjustmentId: string,
): Effect.Effect<
  AdjutantAdjustmentRequest | null,
  AdjutantAdjustmentStorageError
> =>
  d1Effect('adjutantAdjustments.readById', () =>
    db
      .prepare(
        `SELECT ${adjustmentSelectColumns}
           FROM adjutant_adjustment_requests
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(adjustmentId)
      .first<AdjutantAdjustmentRow>(),
  ).pipe(Effect.map(row => (row === null ? null : adjustmentFromRow(row))))

const createAdjustment = (
  db: D1Database,
  runtime: AdjutantAdjustmentRuntime,
  input: CreateAdjutantAdjustmentInput,
): Effect.Effect<AdjutantAdjustmentRequest, AdjutantAdjustmentError> =>
  Effect.gen(function* () {
    const instruction = yield* assertInstructionSafe(input.instruction)
    const now = runtime.nowIso()
    const id = runtime.makeAdjustmentId()

    yield* d1Effect('adjutantAdjustments.insert', () =>
      db
        .prepare(
          `INSERT INTO adjutant_adjustment_requests
             (id,
              assignment_id,
              software_order_id,
              site_id,
              goal_id,
              requested_by_user_id,
              instruction,
              status,
              continuation_mode,
              source_run_id,
              continuation_run_id,
              resulting_version_id,
              visibility,
              created_at,
              updated_at,
              completed_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, 'requested', NULL, ?, NULL, NULL, ?, ?, ?, NULL, NULL)`,
        )
        .bind(
          id,
          input.assignmentId,
          nullableInput(input.softwareOrderId),
          input.siteId,
          nullableInput(input.goalId),
          nullableInput(input.requestedByUserId),
          instruction,
          nullableInput(input.sourceRunId),
          input.visibility ?? 'team',
          now,
          now,
        )
        .run(),
    )

    const adjustment = yield* readAdjustmentById(db, id)

    if (adjustment === null) {
      return yield* new AdjutantAdjustmentNotFound({ adjustmentId: id })
    }

    return adjustment
  })

const listAdjustmentsForAssignment = (
  db: D1Database,
  assignmentId: string,
  limit: number,
): Effect.Effect<
  ReadonlyArray<AdjutantAdjustmentRequest>,
  AdjutantAdjustmentStorageError
> =>
  d1Effect('adjutantAdjustments.listForAssignment', () =>
    db
      .prepare(
        `SELECT ${adjustmentSelectColumns}
           FROM adjutant_adjustment_requests
          WHERE assignment_id = ?
            AND archived_at IS NULL
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(assignmentId, Math.max(1, Math.min(100, Math.trunc(limit))))
      .all<AdjutantAdjustmentRow>(),
  ).pipe(
    Effect.map(result => result.results.map(row => adjustmentFromRow(row))),
  )

const updateAdjustment = (
  db: D1Database,
  runtime: AdjutantAdjustmentRuntime,
  input: UpdateAdjutantAdjustmentInput,
): Effect.Effect<AdjutantAdjustmentRequest, AdjutantAdjustmentError> =>
  Effect.gen(function* () {
    const current = yield* readAdjustmentById(db, input.adjustmentId)

    if (current === null) {
      return yield* new AdjutantAdjustmentNotFound({
        adjustmentId: input.adjustmentId,
      })
    }

    const now = runtime.nowIso()
    const status = input.status ?? current.status
    const completedAt =
      status === 'completed' ||
      status === 'rejected' ||
      status === 'canceled' ||
      status === 'failed'
        ? (current.completedAt ?? now)
        : current.completedAt
    const continuationMode =
      input.continuationMode === undefined
        ? current.continuationMode
        : input.continuationMode
    const continuationRunId =
      input.continuationRunId === undefined
        ? current.continuationRunId
        : nullableInput(input.continuationRunId)
    const resultingVersionId =
      input.resultingVersionId === undefined
        ? current.resultingVersionId
        : nullableInput(input.resultingVersionId)

    yield* d1Effect('adjutantAdjustments.update', () =>
      db
        .prepare(
          `UPDATE adjutant_adjustment_requests
              SET status = ?,
                  continuation_mode = ?,
                  continuation_run_id = ?,
                  resulting_version_id = ?,
                  updated_at = ?,
                  completed_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(
          status,
          continuationMode,
          continuationRunId,
          resultingVersionId,
          now,
          completedAt,
          input.adjustmentId,
        )
        .run(),
    )

    const updated = yield* readAdjustmentById(db, input.adjustmentId)

    if (updated === null) {
      return yield* new AdjutantAdjustmentNotFound({
        adjustmentId: input.adjustmentId,
      })
    }

    return updated
  })

export const makeAdjutantAdjustmentService = (
  db: D1Database,
  runtime: AdjutantAdjustmentRuntime = systemAdjutantAdjustmentRuntime,
) => ({
  createAdjustment: Effect.fn('AdjutantAdjustmentService.createAdjustment')(
    (input: CreateAdjutantAdjustmentInput) =>
      createAdjustment(db, runtime, input),
  ),
  listAdjustmentsForAssignment: Effect.fn(
    'AdjutantAdjustmentService.listAdjustmentsForAssignment',
  )((assignmentId: string, limit: number) =>
    listAdjustmentsForAssignment(db, assignmentId, limit),
  ),
  updateAdjustment: Effect.fn('AdjutantAdjustmentService.updateAdjustment')(
    (input: UpdateAdjutantAdjustmentInput) =>
      updateAdjustment(db, runtime, input),
  ),
})
