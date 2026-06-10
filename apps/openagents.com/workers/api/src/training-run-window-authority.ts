import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'
import { parseJsonStringArray } from './json-boundary'
import { isoTimestampAfterIso } from './runtime-primitives'

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)
const PublicSafeRefs = S.optionalKey(S.Array(PublicSafeRef))
const PublicSafePylonRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(120),
  S.isPattern(/^[a-z0-9][a-z0-9_.:-]*$/),
)

export const TrainingRunState = S.Literals([
  'planned',
  'active',
  'sealed',
  'reconciled',
])
export type TrainingRunState = typeof TrainingRunState.Type

export const TrainingWindowState = S.Literals([
  'planned',
  'active',
  'sealed',
  'reconciled',
])
export type TrainingWindowState = typeof TrainingWindowState.Type

export const TrainingWindowHomeworkKind = S.Literals([
  'admin_dispatched_homework',
  'operator_planned_homework',
  'auto_starter',
])
export type TrainingWindowHomeworkKind = typeof TrainingWindowHomeworkKind.Type

export const TrainingRunPlanRequest = S.Struct({
  promiseRef: PublicSafeRef,
  receiptRefs: PublicSafeRefs,
  sourceRefs: PublicSafeRefs,
  trainingRunRef: S.optionalKey(PublicSafeRef),
})
export type TrainingRunPlanRequest = typeof TrainingRunPlanRequest.Type

export const TrainingWindowPlanRequest = S.Struct({
  datasetRefs: PublicSafeRefs,
  homeworkKind: S.optionalKey(TrainingWindowHomeworkKind),
  priority: S.optionalKey(S.Number),
  receiptRefs: PublicSafeRefs,
  sourceRefs: PublicSafeRefs,
  trainingRunRef: PublicSafeRef,
  windowRef: S.optionalKey(PublicSafeRef),
})
export type TrainingWindowPlanRequest = typeof TrainingWindowPlanRequest.Type

export const TrainingWindowTransitionRequest = S.Struct({
  actorRef: S.optionalKey(PublicSafeRef),
  receiptRef: PublicSafeRef,
})
export type TrainingWindowTransitionRequest =
  typeof TrainingWindowTransitionRequest.Type

export const TrainingWindowLeaseClaimRequest = S.Struct({
  leaseSeconds: S.optionalKey(S.Number),
  pylonRef: PublicSafePylonRef,
  receiptRefs: PublicSafeRefs,
})
export type TrainingWindowLeaseClaimRequest =
  typeof TrainingWindowLeaseClaimRequest.Type

export type TrainingRunRecord = Readonly<{
  createdAt: string
  id: string
  promiseRef: string
  publicProjectionJson: string
  receiptRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  state: TrainingRunState
  trainingRunRef: string
  updatedAt: string
}>

export type TrainingWindowRecord = Readonly<{
  activatedAt: string | null
  datasetRefs: ReadonlyArray<string>
  homeworkKind: TrainingWindowHomeworkKind
  id: string
  plannedAt: string
  priority: number
  publicProjectionJson: string
  receiptRefs: ReadonlyArray<string>
  reconciledAt: string | null
  sealedAt: string | null
  sourceRefs: ReadonlyArray<string>
  state: TrainingWindowState
  trainingRunRef: string
  updatedAt: string
  windowRef: string
}>

export type TrainingWindowLeaseRecord = Readonly<{
  claimedAt: string
  id: string
  leaseExpiresAt: string
  leaseRef: string
  publicProjectionJson: string
  pylonRef: string
  receiptRefs: ReadonlyArray<string>
  state: 'active' | 'released'
  trainingRunRef: string
  windowRef: string
}>

export type TrainingWindowEventRecord = Readonly<{
  actorRef: string
  createdAt: string
  id: string
  receiptRef: string
  stateFrom: TrainingWindowState | null
  stateTo: TrainingWindowState
  transitionKind: string
  windowRef: string
}>

export type TrainingRunProjection = Readonly<{
  createdAtDisplay: string
  promiseRef: string
  receiptRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  state: TrainingRunState
  trainingRunRef: string
  updatedAtDisplay: string
}>

export type TrainingWindowProjection = Readonly<{
  datasetRefs: ReadonlyArray<string>
  homeworkKind: TrainingWindowHomeworkKind
  plannedAtDisplay: string
  priority: number
  receiptRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
  state: TrainingWindowState
  trainingRunRef: string
  updatedAtDisplay: string
  windowRef: string
}>

export type TrainingWindowLeaseProjection = Readonly<{
  claimedAtDisplay: string
  leaseExpiresInSeconds: number
  leaseRef: string
  pylonRef: string
  receiptRefs: ReadonlyArray<string>
  state: 'active' | 'released'
  trainingRunRef: string
  windowRef: string
}>

export type TrainingAuthorityStore = Readonly<{
  claimLease: (
    lease: TrainingWindowLeaseRecord,
    nowIso: string,
  ) => Promise<TrainingWindowLeaseRecord>
  listClaimableWindows: (
    nowIso: string,
    limit: number,
  ) => Promise<ReadonlyArray<TrainingWindowRecord>>
  planRun: (run: TrainingRunRecord) => Promise<TrainingRunRecord>
  planWindow: (window: TrainingWindowRecord) => Promise<TrainingWindowRecord>
  readRun: (trainingRunRef: string) => Promise<TrainingRunRecord | undefined>
  readWindow: (windowRef: string) => Promise<TrainingWindowRecord | undefined>
  transitionWindow: (
    window: TrainingWindowRecord,
    event: TrainingWindowEventRecord,
  ) => Promise<TrainingWindowRecord>
}>

export class TrainingAuthorityStoreError extends S.TaggedErrorClass<TrainingAuthorityStoreError>()(
  'TrainingAuthorityStoreError',
  {
    kind: S.Literals([
      'conflict',
      'forbidden',
      'not_found',
      'storage_error',
      'validation_error',
    ]),
    reason: S.String,
  },
) {}

export type TrainingRunRow = Readonly<{
  created_at: string
  id: string
  promise_ref: string
  public_projection_json: string
  receipt_refs_json: string
  source_refs_json: string
  state: TrainingRunState
  training_run_ref: string
  updated_at: string
}>

export type TrainingWindowRow = Readonly<{
  activated_at: string | null
  dataset_refs_json: string
  homework_kind: TrainingWindowHomeworkKind
  id: string
  planned_at: string
  priority: number
  public_projection_json: string
  receipt_refs_json: string
  reconciled_at: string | null
  sealed_at: string | null
  source_refs_json: string
  state: TrainingWindowState
  training_run_ref: string
  updated_at: string
  window_ref: string
}>

const uniqueRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set((refs ?? []).map(ref => ref.trim()).filter(ref => ref !== '')),
  ].sort()

const homeworkKindRank = (kind: TrainingWindowHomeworkKind): number =>
  kind === 'admin_dispatched_homework'
    ? 3
    : kind === 'operator_planned_homework'
      ? 2
      : 1

export const selectTrainingLeaseCandidate = (
  windows: ReadonlyArray<TrainingWindowRecord>,
): TrainingWindowRecord | undefined =>
  [...windows].sort((left, right) => {
    const rankDelta =
      homeworkKindRank(right.homeworkKind) - homeworkKindRank(left.homeworkKind)

    if (rankDelta !== 0) {
      return rankDelta
    }

    const priorityDelta = right.priority - left.priority

    if (priorityDelta !== 0) {
      return priorityDelta
    }

    return left.plannedAt.localeCompare(right.plannedAt)
  })[0]

export const publicTrainingRunProjection = (
  record: TrainingRunRecord,
  nowIso: string,
): TrainingRunProjection => ({
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.createdAt,
    nowIso,
  ),
  promiseRef: record.promiseRef,
  receiptRefs: uniqueRefs(record.receiptRefs),
  sourceRefs: uniqueRefs(record.sourceRefs),
  state: record.state,
  trainingRunRef: record.trainingRunRef,
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.updatedAt,
    nowIso,
  ),
})

export const publicTrainingWindowProjection = (
  record: TrainingWindowRecord,
  nowIso: string,
): TrainingWindowProjection => ({
  datasetRefs: uniqueRefs(record.datasetRefs),
  homeworkKind: record.homeworkKind,
  plannedAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.plannedAt,
    nowIso,
  ),
  priority: record.priority,
  receiptRefs: uniqueRefs(record.receiptRefs),
  sourceRefs: uniqueRefs(record.sourceRefs),
  state: record.state,
  trainingRunRef: record.trainingRunRef,
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.updatedAt,
    nowIso,
  ),
  windowRef: record.windowRef,
})

export const publicTrainingWindowLeaseProjection = (
  record: TrainingWindowLeaseRecord,
  nowIso: string,
): TrainingWindowLeaseProjection => ({
  claimedAtDisplay: friendlyBlueprintMissionBriefingTime(
    record.claimedAt,
    nowIso,
  ),
  leaseExpiresInSeconds: Math.max(
    0,
    Math.floor((Date.parse(record.leaseExpiresAt) - Date.parse(nowIso)) / 1000),
  ),
  leaseRef: record.leaseRef,
  pylonRef: record.pylonRef,
  receiptRefs: uniqueRefs(record.receiptRefs),
  state: record.state,
  trainingRunRef: record.trainingRunRef,
  windowRef: record.windowRef,
})

export const buildTrainingRunRecord = (
  input: Readonly<{
    makeId: () => string
    nowIso: string
    request: TrainingRunPlanRequest
  }>,
): TrainingRunRecord => {
  const id = input.makeId()
  const record: TrainingRunRecord = {
    createdAt: input.nowIso,
    id: `training_run_${id}`,
    promiseRef: input.request.promiseRef,
    publicProjectionJson: '{}',
    receiptRefs: uniqueRefs(input.request.receiptRefs),
    sourceRefs: uniqueRefs(input.request.sourceRefs),
    state: 'planned',
    trainingRunRef: input.request.trainingRunRef ?? `training.run.${id}`,
    updatedAt: input.nowIso,
  }

  return {
    ...record,
    publicProjectionJson: JSON.stringify(
      publicTrainingRunProjection(record, input.nowIso),
    ),
  }
}

export const buildTrainingWindowRecord = (
  input: Readonly<{
    makeId: () => string
    nowIso: string
    request: TrainingWindowPlanRequest
  }>,
): TrainingWindowRecord => {
  const id = input.makeId()
  const record: TrainingWindowRecord = {
    activatedAt: null,
    datasetRefs: uniqueRefs(input.request.datasetRefs),
    homeworkKind: input.request.homeworkKind ?? 'operator_planned_homework',
    id: `training_window_${id}`,
    plannedAt: input.nowIso,
    priority: Math.trunc(input.request.priority ?? 0),
    publicProjectionJson: '{}',
    receiptRefs: uniqueRefs(input.request.receiptRefs),
    reconciledAt: null,
    sealedAt: null,
    sourceRefs: uniqueRefs(input.request.sourceRefs),
    state: 'planned',
    trainingRunRef: input.request.trainingRunRef,
    updatedAt: input.nowIso,
    windowRef: input.request.windowRef ?? `training.window.${id}`,
  }

  return {
    ...record,
    publicProjectionJson: JSON.stringify(
      publicTrainingWindowProjection(record, input.nowIso),
    ),
  }
}

export const transitionTrainingWindowRecord = (
  input: Readonly<{
    actorRef: string
    eventId: string
    nextState: TrainingWindowState
    nowIso: string
    receiptRef: string
    transitionKind: string
    window: TrainingWindowRecord
  }>,
): Readonly<{
  event: TrainingWindowEventRecord
  window: TrainingWindowRecord
}> => {
  const allowed =
    (input.window.state === 'planned' && input.nextState === 'active') ||
    (input.window.state === 'active' && input.nextState === 'sealed') ||
    (input.window.state === 'sealed' && input.nextState === 'reconciled')

  if (!allowed) {
    throw new TrainingAuthorityStoreError({
      kind: 'conflict',
      reason: `Cannot transition training window from ${input.window.state} to ${input.nextState}.`,
    })
  }

  const nextWindow: TrainingWindowRecord = {
    ...input.window,
    activatedAt:
      input.nextState === 'active' ? input.nowIso : input.window.activatedAt,
    receiptRefs: uniqueRefs([...input.window.receiptRefs, input.receiptRef]),
    reconciledAt:
      input.nextState === 'reconciled'
        ? input.nowIso
        : input.window.reconciledAt,
    sealedAt:
      input.nextState === 'sealed' ? input.nowIso : input.window.sealedAt,
    state: input.nextState,
    updatedAt: input.nowIso,
  }

  return {
    event: {
      actorRef: input.actorRef,
      createdAt: input.nowIso,
      id: `training_window_event_${input.eventId}`,
      receiptRef: input.receiptRef,
      stateFrom: input.window.state,
      stateTo: input.nextState,
      transitionKind: input.transitionKind,
      windowRef: input.window.windowRef,
    },
    window: {
      ...nextWindow,
      publicProjectionJson: JSON.stringify(
        publicTrainingWindowProjection(nextWindow, input.nowIso),
      ),
    },
  }
}

const leaseSecondsForRequest = (
  request: TrainingWindowLeaseClaimRequest,
): number => {
  const leaseSeconds = request.leaseSeconds ?? 15 * 60

  if (
    !Number.isFinite(leaseSeconds) ||
    leaseSeconds < 60 ||
    leaseSeconds > 86_400
  ) {
    throw new TrainingAuthorityStoreError({
      kind: 'validation_error',
      reason: 'leaseSeconds must be between 60 and 86400.',
    })
  }

  return Math.floor(leaseSeconds)
}

export const buildTrainingWindowLeaseRecord = (
  input: Readonly<{
    makeId: () => string
    nowIso: string
    request: TrainingWindowLeaseClaimRequest
    window: TrainingWindowRecord
  }>,
): TrainingWindowLeaseRecord => {
  const id = input.makeId()
  const record: TrainingWindowLeaseRecord = {
    claimedAt: input.nowIso,
    id: `training_window_lease_${id}`,
    leaseExpiresAt: isoTimestampAfterIso(
      input.nowIso,
      leaseSecondsForRequest(input.request) * 1000,
    ),
    leaseRef: `training.lease.${id}`,
    publicProjectionJson: '{}',
    pylonRef: input.request.pylonRef,
    receiptRefs: uniqueRefs(input.request.receiptRefs),
    state: 'active',
    trainingRunRef: input.window.trainingRunRef,
    windowRef: input.window.windowRef,
  }

  return {
    ...record,
    publicProjectionJson: JSON.stringify(
      publicTrainingWindowLeaseProjection(record, input.nowIso),
    ),
  }
}

export const trainingAuthorityStoreErrorFromUnknown = (
  error: unknown,
): TrainingAuthorityStoreError =>
  error instanceof TrainingAuthorityStoreError
    ? error
    : new TrainingAuthorityStoreError({
        kind: 'storage_error',
        reason: error instanceof Error ? error.message : String(error),
      })

export const rowToTrainingRun = (row: TrainingRunRow): TrainingRunRecord => ({
  createdAt: row.created_at,
  id: row.id,
  promiseRef: row.promise_ref,
  publicProjectionJson: row.public_projection_json,
  receiptRefs: parseJsonStringArray(row.receipt_refs_json),
  sourceRefs: parseJsonStringArray(row.source_refs_json),
  state: row.state,
  trainingRunRef: row.training_run_ref,
  updatedAt: row.updated_at,
})

export const rowToTrainingWindow = (
  row: TrainingWindowRow,
): TrainingWindowRecord => ({
  activatedAt: row.activated_at,
  datasetRefs: parseJsonStringArray(row.dataset_refs_json),
  homeworkKind: row.homework_kind,
  id: row.id,
  plannedAt: row.planned_at,
  priority: row.priority,
  publicProjectionJson: row.public_projection_json,
  receiptRefs: parseJsonStringArray(row.receipt_refs_json),
  reconciledAt: row.reconciled_at,
  sealedAt: row.sealed_at,
  sourceRefs: parseJsonStringArray(row.source_refs_json),
  state: row.state,
  trainingRunRef: row.training_run_ref,
  updatedAt: row.updated_at,
  windowRef: row.window_ref,
})

export const makeD1TrainingAuthorityStore = (
  db: D1Database,
): TrainingAuthorityStore => ({
  claimLease: async lease => {
    await db
      .prepare(
        `INSERT INTO training_window_leases
          (id, lease_ref, window_ref, training_run_ref, pylon_ref, state,
           receipt_refs_json, public_projection_json, claimed_at,
           lease_expires_at, archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        lease.id,
        lease.leaseRef,
        lease.windowRef,
        lease.trainingRunRef,
        lease.pylonRef,
        lease.state,
        JSON.stringify(lease.receiptRefs),
        lease.publicProjectionJson,
        lease.claimedAt,
        lease.leaseExpiresAt,
      )
      .run()

    return lease
  },
  listClaimableWindows: async (nowIso, limit) => {
    const result = await db
      .prepare(
        `SELECT w.*
           FROM training_windows w
          WHERE w.state = 'active'
            AND w.archived_at IS NULL
            AND NOT EXISTS (
              SELECT 1
                FROM training_window_leases l
               WHERE l.window_ref = w.window_ref
                 AND l.state = 'active'
                 AND l.lease_expires_at > ?
                 AND l.archived_at IS NULL
            )
          ORDER BY
            CASE w.homework_kind
              WHEN 'admin_dispatched_homework' THEN 3
              WHEN 'operator_planned_homework' THEN 2
              ELSE 1
            END DESC,
            w.priority DESC,
            w.planned_at ASC
          LIMIT ?`,
      )
      .bind(nowIso, limit)
      .all<TrainingWindowRow>()

    return (result.results ?? []).map(rowToTrainingWindow)
  },
  planRun: async run => {
    await db
      .prepare(
        `INSERT INTO training_runs
          (id, training_run_ref, promise_ref, state, source_refs_json,
           receipt_refs_json, public_projection_json, created_at, updated_at,
           archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        run.id,
        run.trainingRunRef,
        run.promiseRef,
        run.state,
        JSON.stringify(run.sourceRefs),
        JSON.stringify(run.receiptRefs),
        run.publicProjectionJson,
        run.createdAt,
        run.updatedAt,
      )
      .run()

    return run
  },
  planWindow: async window => {
    await db
      .prepare(
        `INSERT INTO training_windows
          (id, window_ref, training_run_ref, state, homework_kind, priority,
           dataset_refs_json, source_refs_json, receipt_refs_json,
           public_projection_json, planned_at, activated_at, sealed_at,
           reconciled_at, updated_at, archived_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        window.id,
        window.windowRef,
        window.trainingRunRef,
        window.state,
        window.homeworkKind,
        window.priority,
        JSON.stringify(window.datasetRefs),
        JSON.stringify(window.sourceRefs),
        JSON.stringify(window.receiptRefs),
        window.publicProjectionJson,
        window.plannedAt,
        window.activatedAt,
        window.sealedAt,
        window.reconciledAt,
        window.updatedAt,
      )
      .run()

    return window
  },
  readRun: async trainingRunRef => {
    const row = await db
      .prepare(
        `SELECT *
           FROM training_runs
          WHERE training_run_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(trainingRunRef)
      .first<TrainingRunRow>()

    return row === null ? undefined : rowToTrainingRun(row)
  },
  readWindow: async windowRef => {
    const row = await db
      .prepare(
        `SELECT *
           FROM training_windows
          WHERE window_ref = ?
            AND archived_at IS NULL`,
      )
      .bind(windowRef)
      .first<TrainingWindowRow>()

    return row === null ? undefined : rowToTrainingWindow(row)
  },
  transitionWindow: async (window, event) => {
    await db.batch([
      db
        .prepare(
          `UPDATE training_windows
              SET state = ?,
                  receipt_refs_json = ?,
                  public_projection_json = ?,
                  activated_at = ?,
                  sealed_at = ?,
                  reconciled_at = ?,
                  updated_at = ?
            WHERE window_ref = ?
              AND archived_at IS NULL`,
        )
        .bind(
          window.state,
          JSON.stringify(window.receiptRefs),
          window.publicProjectionJson,
          window.activatedAt,
          window.sealedAt,
          window.reconciledAt,
          window.updatedAt,
          window.windowRef,
        ),
      db
        .prepare(
          `INSERT INTO training_window_events
            (id, window_ref, transition_kind, state_from, state_to, actor_ref,
             receipt_ref, created_at, archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          event.id,
          event.windowRef,
          event.transitionKind,
          event.stateFrom,
          event.stateTo,
          event.actorRef,
          event.receiptRef,
          event.createdAt,
        ),
    ])

    return window
  },
})
