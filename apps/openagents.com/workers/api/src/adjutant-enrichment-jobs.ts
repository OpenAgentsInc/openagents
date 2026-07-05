import { redactProviderAccountSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import type { AdjutantAssignment } from './adjutant-assignments'
import {
  type ExaEnrichmentPlan,
  makeAdjutantEnrichmentPlanner,
} from './adjutant-enrichment-planner'
import { makeAdjutantEnrichmentLedger } from './adjutant-enrichment-ledger'
import { makeAdjutantPublicSourceRefService } from './adjutant-public-source-refs'
import { parseJsonRecord } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import type { SupervisionLongtailMirror } from './supervision-longtail-domain-store'

export const AdjutantEnrichmentJobStatus = S.Literals([
  'queued',
  'running',
  'succeeded',
  'failed',
  'skipped',
  'canceled',
])
export type AdjutantEnrichmentJobStatus =
  typeof AdjutantEnrichmentJobStatus.Type

export const AdjutantEnrichmentJobTrigger = S.Literals([
  'research_required',
  'operator_requested',
  'operator_refresh',
])
export type AdjutantEnrichmentJobTrigger =
  typeof AdjutantEnrichmentJobTrigger.Type

export class AdjutantEnrichmentQueueMessage extends S.Class<AdjutantEnrichmentQueueMessage>(
  'AdjutantEnrichmentQueueMessage',
)({
  assignmentId: S.String,
  jobId: S.String,
  schemaVersion: S.Literal('openagents.adjutant_enrichment_job.v1'),
}) {}

export const EnqueueAdjutantEnrichmentJobInput = S.Struct({
  assignment: S.Unknown,
  freshnessMaxAgeHours: S.optionalKey(S.Number),
  numResults: S.optionalKey(S.Number),
  operatorNotes: S.optionalKey(S.String),
  refresh: S.optionalKey(S.Boolean),
  requestBudget: S.optionalKey(S.Number),
  requestedByUserId: S.optionalKey(S.NullOr(S.String)),
  triggerKind: AdjutantEnrichmentJobTrigger,
})
export type EnqueueAdjutantEnrichmentJobInput = Readonly<{
  assignment: AdjutantAssignment
  freshnessMaxAgeHours?: number | undefined
  numResults?: number | undefined
  operatorNotes?: string | undefined
  refresh?: boolean | undefined
  requestBudget?: number | undefined
  requestedByUserId?: string | null | undefined
  triggerKind: AdjutantEnrichmentJobTrigger
}>

export type AdjutantEnrichmentJob = Readonly<{
  assignmentId: string
  completedAt: string | null
  createdAt: string
  enrichmentRunId: string | null
  errorCode: string | null
  errorSummary: string | null
  id: string
  refresh: boolean
  requestedByUserId: string | null
  request: Record<string, unknown>
  startedAt: string | null
  status: AdjutantEnrichmentJobStatus
  triggerKind: AdjutantEnrichmentJobTrigger
  updatedAt: string
}>

type JobRow = Readonly<{
  assignment_id: string
  completed_at: string | null
  created_at: string
  enrichment_run_id: string | null
  error_code: string | null
  error_summary: string | null
  id: string
  refresh: number
  requested_by_user_id: string | null
  request_json: string | null
  started_at: string | null
  status: AdjutantEnrichmentJobStatus
  trigger_kind: AdjutantEnrichmentJobTrigger
  updated_at: string
}>

export type AdjutantEnrichmentJobRuntime = Readonly<{
  makeJobId: () => string
  nowIso: () => string
}>

export const systemAdjutantEnrichmentJobRuntime: AdjutantEnrichmentJobRuntime =
  {
    makeJobId: () => compactRandomId('adjutant_enrichment_job'),
    nowIso: currentIsoTimestamp,
  }

export class AdjutantEnrichmentJobActiveExists extends S.TaggedErrorClass<AdjutantEnrichmentJobActiveExists>()(
  'AdjutantEnrichmentJobActiveExists',
  {
    assignmentId: S.String,
    jobId: S.String,
  },
) {}

export class AdjutantEnrichmentJobNotFound extends S.TaggedErrorClass<AdjutantEnrichmentJobNotFound>()(
  'AdjutantEnrichmentJobNotFound',
  {
    jobId: S.String,
  },
) {}

export class AdjutantEnrichmentJobStorageError extends S.TaggedErrorClass<AdjutantEnrichmentJobStorageError>()(
  'AdjutantEnrichmentJobStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class AdjutantEnrichmentJobUnsafePayload extends S.TaggedErrorClass<AdjutantEnrichmentJobUnsafePayload>()(
  'AdjutantEnrichmentJobUnsafePayload',
  {
    reason: S.String,
  },
) {}

export type AdjutantEnrichmentJobError =
  | AdjutantEnrichmentJobActiveExists
  | AdjutantEnrichmentJobNotFound
  | AdjutantEnrichmentJobStorageError
  | AdjutantEnrichmentJobUnsafePayload

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AdjutantEnrichmentJobStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new AdjutantEnrichmentJobStorageError({ operation, error }),
  })

const jobFromRow = (row: JobRow): AdjutantEnrichmentJob => ({
  assignmentId: row.assignment_id,
  completedAt: row.completed_at,
  createdAt: row.created_at,
  enrichmentRunId: row.enrichment_run_id,
  errorCode: row.error_code,
  errorSummary: row.error_summary,
  id: row.id,
  refresh: row.refresh === 1,
  requestedByUserId: row.requested_by_user_id,
  request: parseJsonRecord(row.request_json) ?? {},
  startedAt: row.started_at,
  status: row.status,
  triggerKind: row.trigger_kind,
  updatedAt: row.updated_at,
})

const readLatestJobForAssignment = (
  db: D1Database,
  assignmentId: string,
): Effect.Effect<AdjutantEnrichmentJob | null, AdjutantEnrichmentJobError> =>
  d1Effect('adjutantEnrichmentJobs.latestForAssignment.read', () =>
    db
      .prepare(
        `SELECT id,
                assignment_id,
                enrichment_run_id,
                status,
                trigger_kind,
                refresh,
                requested_by_user_id,
                request_json,
                error_code,
                error_summary,
                started_at,
                completed_at,
                created_at,
                updated_at
           FROM adjutant_enrichment_jobs
          WHERE assignment_id = ?
            AND archived_at IS NULL
          ORDER BY updated_at DESC
          LIMIT 1`,
      )
      .bind(assignmentId)
      .first<JobRow>(),
  ).pipe(Effect.map(row => (row === null ? null : jobFromRow(row))))

const readJobById = (
  db: D1Database,
  jobId: string,
): Effect.Effect<AdjutantEnrichmentJob, AdjutantEnrichmentJobError> =>
  d1Effect('adjutantEnrichmentJobs.byId.read', () =>
    db
      .prepare(
        `SELECT id,
                assignment_id,
                enrichment_run_id,
                status,
                trigger_kind,
                refresh,
                requested_by_user_id,
                request_json,
                error_code,
                error_summary,
                started_at,
                completed_at,
                created_at,
                updated_at
           FROM adjutant_enrichment_jobs
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(jobId)
      .first<JobRow>(),
  ).pipe(
    Effect.flatMap(row =>
      row === null
        ? Effect.fail(new AdjutantEnrichmentJobNotFound({ jobId }))
        : Effect.succeed(jobFromRow(row)),
    ),
  )

const readActiveJobForAssignment = (
  db: D1Database,
  assignmentId: string,
): Effect.Effect<AdjutantEnrichmentJob | null, AdjutantEnrichmentJobError> =>
  d1Effect('adjutantEnrichmentJobs.activeForAssignment.read', () =>
    db
      .prepare(
        `SELECT id,
                assignment_id,
                enrichment_run_id,
                status,
                trigger_kind,
                refresh,
                requested_by_user_id,
                request_json,
                error_code,
                error_summary,
                started_at,
                completed_at,
                created_at,
                updated_at
           FROM adjutant_enrichment_jobs
          WHERE assignment_id = ?
            AND archived_at IS NULL
            AND status IN ('queued', 'running')
          ORDER BY updated_at DESC
          LIMIT 1`,
      )
      .bind(assignmentId)
      .first<JobRow>(),
  ).pipe(Effect.map(row => (row === null ? null : jobFromRow(row))))

const buildPlan = (
  db: D1Database,
  input: EnqueueAdjutantEnrichmentJobInput,
): Effect.Effect<ExaEnrichmentPlan, AdjutantEnrichmentJobError> =>
  Effect.gen(function* () {
    const explicitSourceRefs = yield* makeAdjutantPublicSourceRefService(
      db,
    ).plannerSourceRefsForAssignment(input.assignment.id).pipe(
      Effect.mapError(
        error =>
          new AdjutantEnrichmentJobStorageError({
            error,
            operation: 'adjutantEnrichmentJobs.sourceRefs.plan',
          }),
      ),
    )

    return yield* makeAdjutantEnrichmentPlanner()
      .buildPlan({
        assignment: input.assignment,
        explicitSourceRefs,
        freshnessMaxAgeHours: input.freshnessMaxAgeHours,
        numResults: input.numResults,
        operatorNotes: input.operatorNotes,
        order: null,
        site: null,
      })
      .pipe(
        Effect.mapError(
          error =>
            new AdjutantEnrichmentJobStorageError({
              error,
              operation: 'adjutantEnrichmentJobs.plan.build',
            }),
        ),
      )
  })

const enqueueJob = (
  db: D1Database,
  runtime: AdjutantEnrichmentJobRuntime,
  input: EnqueueAdjutantEnrichmentJobInput,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<
  Readonly<{
    duplicate: boolean
    job: AdjutantEnrichmentJob
    plan: ExaEnrichmentPlan | null
  }>,
  AdjutantEnrichmentJobError
> =>
  Effect.gen(function* () {
    const active = yield* readActiveJobForAssignment(db, input.assignment.id)

    if (active !== null) {
      return { duplicate: true, job: active, plan: null }
    }

    const plan = yield* buildPlan(db, input)
    const request = {
      freshnessMaxAgeHours: input.freshnessMaxAgeHours ?? null,
      numResults: input.numResults ?? null,
      operatorNotes: input.operatorNotes ?? null,
      requestBudget: input.requestBudget ?? null,
    }
    const requestJson = JSON.stringify(request)

    if (redactProviderAccountSecretMaterial(requestJson) !== requestJson) {
      return yield* new AdjutantEnrichmentJobUnsafePayload({
        reason: 'Enrichment job request contains secret-shaped material.',
      })
    }

    const allTasks = [...plan.searchTasks, ...plan.contentsTasks]
    const requestBudget = Math.max(
      0,
      Math.trunc(input.requestBudget ?? Math.min(6, allTasks.length)),
    )
    const run = yield* makeAdjutantEnrichmentLedger(db, undefined, mirror)
      .createRun({
        assignmentId: input.assignment.id,
        planId: plan.planId,
        requestBudget,
        siteId: input.assignment.siteId,
        softwareOrderId: input.assignment.softwareOrderId,
        status: 'queued',
        subject: plan.subjectSummary,
      })
      .pipe(
        Effect.mapError(
          error =>
            new AdjutantEnrichmentJobStorageError({
              error,
              operation: 'adjutantEnrichmentJobs.run.createQueued',
            }),
        ),
      )
    yield* makeAdjutantEnrichmentLedger(db, undefined, mirror)
      .linkAssignmentRun({
        assignmentId: input.assignment.id,
        enrichmentRunId: run.id,
        requiredForLaunch: true,
        status: 'planned',
      })
      .pipe(
        Effect.mapError(
          error =>
            new AdjutantEnrichmentJobStorageError({
              error,
              operation: 'adjutantEnrichmentJobs.run.linkQueued',
            }),
        ),
      )

    const now = runtime.nowIso()
    const jobId = runtime.makeJobId()

    yield* d1Effect('adjutantEnrichmentJobs.insert', () =>
      db
        .prepare(
          `INSERT INTO adjutant_enrichment_jobs
             (id,
              assignment_id,
              enrichment_run_id,
              status,
              trigger_kind,
              refresh,
              requested_by_user_id,
              request_json,
              created_at,
              updated_at)
           VALUES (?, ?, ?, 'queued', ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          jobId,
          input.assignment.id,
          run.id,
          input.triggerKind,
          input.refresh === true ? 1 : 0,
          input.requestedByUserId ?? null,
          requestJson,
          now,
          now,
        )
        .run(),
    )

    if (mirror !== undefined) {
      yield* Effect.promise(() =>
        mirror.mirrorRowsByKey('adjutant_enrichment_jobs', [[jobId]]),
      )
    }

    const job = yield* readLatestJobForAssignment(db, input.assignment.id)

    if (job === null) {
      return yield* new AdjutantEnrichmentJobStorageError({
        error: new Error('Queued enrichment job was not readable after insert.'),
        operation: 'adjutantEnrichmentJobs.readAfterInsert',
      })
    }

    return { duplicate: false, job, plan }
  })

const updateJobStatus = (
  db: D1Database,
  runtime: AdjutantEnrichmentJobRuntime,
  input: Readonly<{
    completed?: boolean | undefined
    errorCode?: string | null | undefined
    errorSummary?: string | null | undefined
    jobId: string
    started?: boolean | undefined
    status: AdjutantEnrichmentJobStatus
  }>,
  mirror?: SupervisionLongtailMirror | undefined,
): Effect.Effect<AdjutantEnrichmentJob, AdjutantEnrichmentJobError> =>
  Effect.gen(function* () {
    const now = runtime.nowIso()
    const errorSummary =
      input.errorSummary === undefined || input.errorSummary === null
        ? null
        : redactProviderAccountSecretMaterial(input.errorSummary).slice(0, 500)

    yield* d1Effect('adjutantEnrichmentJobs.status.update', () =>
      db
        .prepare(
          `UPDATE adjutant_enrichment_jobs
              SET status = ?,
                  error_code = ?,
                  error_summary = ?,
                  started_at = CASE
                    WHEN ? = 1 AND started_at IS NULL THEN ?
                    ELSE started_at
                  END,
                  completed_at = CASE
                    WHEN ? = 1 THEN ?
                    ELSE completed_at
                  END,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(
          input.status,
          input.errorCode ?? null,
          errorSummary,
          input.started === true ? 1 : 0,
          now,
          input.completed === true ? 1 : 0,
          now,
          now,
          input.jobId,
        )
        .run(),
    )

    const row = yield* d1Effect('adjutantEnrichmentJobs.byId.read', () =>
      db
        .prepare(
          `SELECT id,
                  assignment_id,
                  enrichment_run_id,
                  status,
                  trigger_kind,
                  refresh,
                  requested_by_user_id,
                  request_json,
                  error_code,
                  error_summary,
                  started_at,
                  completed_at,
                  created_at,
                  updated_at
             FROM adjutant_enrichment_jobs
            WHERE id = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(input.jobId)
        .first<JobRow>(),
    )

    if (row === null) {
      return yield* new AdjutantEnrichmentJobNotFound({ jobId: input.jobId })
    }

    if (mirror !== undefined) {
      yield* Effect.promise(() =>
        mirror.mirrorRowsByKey('adjutant_enrichment_jobs', [[input.jobId]]),
      )
    }

    return jobFromRow(row)
  })

export const makeAdjutantEnrichmentJobService = (
  db: D1Database,
  runtime: AdjutantEnrichmentJobRuntime = systemAdjutantEnrichmentJobRuntime,
  mirror?: SupervisionLongtailMirror | undefined,
) => ({
  enqueueJob: Effect.fn('AdjutantEnrichmentJobService.enqueueJob')(
    (input: EnqueueAdjutantEnrichmentJobInput) =>
      enqueueJob(db, runtime, input, mirror),
  ),
  latestJobForAssignment: Effect.fn(
    'AdjutantEnrichmentJobService.latestJobForAssignment',
  )((assignmentId: string) => readLatestJobForAssignment(db, assignmentId)),
  readJobById: Effect.fn('AdjutantEnrichmentJobService.readJobById')(
    (jobId: string) => readJobById(db, jobId),
  ),
  updateJobStatus: Effect.fn('AdjutantEnrichmentJobService.updateJobStatus')(
    (input: Parameters<typeof updateJobStatus>[2]) =>
      updateJobStatus(db, runtime, input, mirror),
  ),
})
