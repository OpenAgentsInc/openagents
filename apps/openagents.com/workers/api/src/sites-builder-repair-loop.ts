import {
  containsProviderSecretMaterial,
  redactProviderAccountSecretMaterial,
} from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import {
  type SiteBuilderEventKind,
  type SiteBuilderPhaseKind,
  type SiteBuilderPhaseStatus,
  type SiteBuilderRuntime,
  SiteBuilderSessionStorageError,
  SiteBuilderSessionValidationError,
  appendSiteBuilderEvent,
  readSiteBuilderSessionProjection,
  systemSiteBuilderRuntime,
} from './sites-builder-sessions'

export const SiteBuilderRepairFailureKind = S.Literals([
  'build_error',
  'runtime_error',
  'preview_error',
  'validation_error',
  'unknown',
])
export type SiteBuilderRepairFailureKind =
  typeof SiteBuilderRepairFailureKind.Type

export const SiteBuilderRepairAttemptRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  attemptNumber: S.Number,
  completedAt: S.NullOr(S.String),
  createdAt: S.String,
  failureKind: SiteBuilderRepairFailureKind,
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  phaseKind: S.NullOr(S.String),
  previewId: S.NullOr(S.String),
  redactedSummary: S.String,
  retryBudget: S.Number,
  sessionId: S.String,
  status: S.String,
  stopReason: S.NullOr(S.String),
})
export type SiteBuilderRepairAttemptRecord =
  typeof SiteBuilderRepairAttemptRecord.Type

export type RecordSiteBuilderRepairAttemptInput = Readonly<{
  attemptNumber?: number | undefined
  completedAt?: string | undefined
  failureKind: SiteBuilderRepairFailureKind
  failureSummary: string
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  phaseKind?: SiteBuilderPhaseKind | undefined
  previewId?: string | undefined
  retryBudget: number
  sessionId: string
  status?: SiteBuilderPhaseStatus | undefined
  stopReason?: string | undefined
}>

type RepairAttemptRow = Readonly<{
  archived_at: string | null
  attempt_number: number
  completed_at: string | null
  created_at: string
  failure_kind: SiteBuilderRepairFailureKind
  id: string
  idempotency_key: string
  metadata_json: string
  phase_kind: string | null
  preview_id: string | null
  redacted_summary: string
  retry_budget: number
  session_id: string
  status: string
  stop_reason: string | null
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const UNSAFE_REPAIR_TEXT =
  /\b(provider[_ -]?payload|runner[_ -]?payload|browser[_ -]?log|access_token|refresh_token|device_auth_id|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic|bypass|captcha|cloudflare challenge|headless stealth)/i

const compactText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !UNSAFE_REPAIR_TEXT.test(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new SiteBuilderSessionValidationError({
      reason: `${field} must be a public-safe repair ref.`,
    })
  }
}

const assertSafeMetadata = (
  metadata: Readonly<Record<string, unknown>> | undefined,
) => {
  const safeMetadata = metadata ?? {}
  const json = JSON.stringify(safeMetadata)

  if (!textIsSafe(json)) {
    throw new SiteBuilderSessionValidationError({
      reason:
        'repair metadata must not contain private runner or secret material.',
    })
  }

  return safeMetadata
}

const redactedRepairSummary = (summary: string): string => {
  const redacted = compactText(
    redactProviderAccountSecretMaterial(summary),
    1000,
  )

  if (redacted === '' || !textIsSafe(redacted)) {
    throw new SiteBuilderSessionValidationError({
      reason:
        'repair failure summary must be bounded and free of private runner or secret material.',
    })
  }

  return redacted
}

const repairAttemptFromRow = (
  row: RepairAttemptRow,
): SiteBuilderRepairAttemptRecord => ({
  archivedAt: row.archived_at,
  attemptNumber: row.attempt_number,
  completedAt: row.completed_at,
  createdAt: row.created_at,
  failureKind: row.failure_kind,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  phaseKind: row.phase_kind,
  previewId: row.preview_id,
  redactedSummary: row.redacted_summary,
  retryBudget: row.retry_budget,
  sessionId: row.session_id,
  status: row.status,
  stopReason: row.stop_reason,
})

const storageError = (operation: string, error: unknown) =>
  new SiteBuilderSessionStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const readByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  SiteBuilderRepairAttemptRecord | null,
  SiteBuilderSessionStorageError
> =>
  Effect.tryPromise({
    try: async () => {
      const row = await db
        .prepare(
          `SELECT *
             FROM site_builder_repair_attempts
            WHERE idempotency_key = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(idempotencyKey)
        .first<RepairAttemptRow>()

      return row === null ? null : repairAttemptFromRow(row)
    },
    catch: error =>
      storageError('siteBuilderRepairAttempt.readByIdempotencyKey', error),
  })

const nextAttemptNumber = (
  db: D1Database,
  sessionId: string,
): Effect.Effect<number, SiteBuilderSessionStorageError> =>
  Effect.tryPromise({
    try: async () => {
      const row = await db
        .prepare(
          `SELECT COALESCE(MAX(attempt_number), 0) + 1 AS next_attempt
             FROM site_builder_repair_attempts
            WHERE session_id = ?
              AND archived_at IS NULL`,
        )
        .bind(sessionId)
        .first<{ next_attempt: number }>()

      return row?.next_attempt ?? 1
    },
    catch: error =>
      storageError('siteBuilderRepairAttempt.nextAttemptNumber', error),
  })

const eventKindForRepairStatus = (
  status: SiteBuilderPhaseStatus,
): SiteBuilderEventKind =>
  status === 'succeeded'
    ? 'build_repaired'
    : status === 'failed' || status === 'blocked'
      ? 'build_failed'
      : 'phase_updated'

const validateRepairAttempt = (
  input: RecordSiteBuilderRepairAttemptInput,
): Readonly<{
  completedAt: string | null
  metadata: Readonly<Record<string, unknown>>
  redactedSummary: string
  status: SiteBuilderPhaseStatus
  stopReason: string | null
}> => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('sessionId', input.sessionId)
  assertSafeRef('previewId', input.previewId)
  assertSafeRef('completedAt', input.completedAt)

  if (!Number.isSafeInteger(input.retryBudget) || input.retryBudget < 1) {
    throw new SiteBuilderSessionValidationError({
      reason: 'retryBudget must be a positive safe integer.',
    })
  }

  if (
    input.attemptNumber !== undefined &&
    (!Number.isSafeInteger(input.attemptNumber) || input.attemptNumber < 1)
  ) {
    throw new SiteBuilderSessionValidationError({
      reason: 'attemptNumber must be a positive safe integer.',
    })
  }

  if (
    input.attemptNumber !== undefined &&
    input.attemptNumber > input.retryBudget
  ) {
    throw new SiteBuilderSessionValidationError({
      reason: 'repair retry budget is exhausted.',
    })
  }

  return {
    completedAt: input.completedAt ?? null,
    metadata: assertSafeMetadata(input.metadata),
    redactedSummary: redactedRepairSummary(input.failureSummary),
    status: input.status ?? 'running',
    stopReason:
      input.stopReason === undefined
        ? null
        : redactedRepairSummary(input.stopReason),
  }
}

const validationEffect = <A>(validate: () => A) =>
  Effect.try({
    try: validate,
    catch: error =>
      error instanceof SiteBuilderSessionValidationError
        ? error
        : new SiteBuilderSessionValidationError({
            reason: error instanceof Error ? error.message : String(error),
          }),
  })

export const recordSiteBuilderRepairAttempt = (
  db: D1Database,
  input: RecordSiteBuilderRepairAttemptInput,
  runtime: SiteBuilderRuntime = systemSiteBuilderRuntime,
): Effect.Effect<
  SiteBuilderRepairAttemptRecord,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    const valid = yield* validationEffect(() => validateRepairAttempt(input))
    const existing = yield* readByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    yield* readSiteBuilderSessionProjection(db, input.sessionId)

    const attemptNumber =
      input.attemptNumber ?? (yield* nextAttemptNumber(db, input.sessionId))

    if (attemptNumber > input.retryBudget) {
      return yield* new SiteBuilderSessionValidationError({
        reason: 'repair retry budget is exhausted.',
      })
    }

    const now = runtime.nowIso()
    const id = input.id ?? runtime.randomId('site_builder_repair')

    yield* Effect.tryPromise({
      try: () =>
        db
          .prepare(
            `INSERT OR IGNORE INTO site_builder_repair_attempts (
               id,
               idempotency_key,
               session_id,
               preview_id,
               phase_kind,
               attempt_number,
               retry_budget,
               status,
               failure_kind,
               redacted_summary,
               stop_reason,
               metadata_json,
               created_at,
               completed_at,
               archived_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
          )
          .bind(
            id,
            input.idempotencyKey,
            input.sessionId,
            input.previewId ?? null,
            input.phaseKind ?? null,
            attemptNumber,
            input.retryBudget,
            valid.status,
            input.failureKind,
            valid.redactedSummary,
            valid.stopReason,
            JSON.stringify(valid.metadata),
            now,
            valid.completedAt,
          )
          .run(),
      catch: error => storageError('siteBuilderRepairAttempt.insert', error),
    })

    const record = yield* readByIdempotencyKey(db, input.idempotencyKey)

    if (record === null) {
      return yield* new SiteBuilderSessionStorageError({
        operation: 'siteBuilderRepairAttempt.readInserted',
        reason: 'site builder repair attempt was not readable after insert.',
      })
    }

    yield* appendSiteBuilderEvent(
      db,
      {
        eventKind: eventKindForRepairStatus(valid.status),
        idempotencyKey: `${input.idempotencyKey}:event`,
        payload: {
          attemptNumber: record.attemptNumber,
          repairAttemptId: record.id,
          retryBudget: record.retryBudget,
        },
        phaseKind: input.phaseKind,
        sessionId: input.sessionId,
        status: valid.status,
        summary: valid.redactedSummary,
        title: 'Repair attempt recorded',
        visibility: 'customer',
      },
      runtime,
    )

    return record
  })
