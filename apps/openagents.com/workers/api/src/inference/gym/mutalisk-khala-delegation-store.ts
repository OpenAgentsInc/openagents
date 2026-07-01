// Durable Worker-side store for the Khala Code delegation GEPA Gym seam (#7799).
//
// The Worker owns only the public-safe workflow/projection boundary:
//   - jobs identify an owner-gated `khala-code-delegation-gepa` run,
//   - progress rows record coarse Mutalisk lifecycle stages,
//   - summaries/admissions persist the safe manifest projection,
//   - projection_json is the compact read model Khala Code can poll.
//
// It never stores raw prompts, traces, local paths, provider payloads, optimizer
// scratch logs, or private endpoints. Reads re-decode JSON blobs through Effect
// schemas so a tampered row is dropped rather than served.
import { Effect, Schema as S } from 'effect'

import { parseJsonWithSchema } from '../../json-boundary'
import {
  assertMutaliskKhalaDelegationPublicSafeValues,
  KhalaCodeDelegationGepaEnvironmentId,
  MutaliskKhalaDelegationBridgeOutput,
  MutaliskKhalaDelegationDemandKind,
  MutaliskKhalaDelegationDemandSource,
  MutaliskKhalaDelegationJob,
  MutaliskKhalaDelegationRunProgress,
  MutaliskKhalaDelegationStage,
  MutaliskKhalaDelegationSummary,
} from './mutalisk-khala-delegation-bridge'

export const MutaliskKhalaDelegationRunProjectionSchemaVersion =
  'openagents.gym.mutalisk_khala_delegation_projection.v0'

export class MutaliskKhalaDelegationRunProjection extends S.Class<MutaliskKhalaDelegationRunProjection>(
  'MutaliskKhalaDelegationRunProjection',
)({
  schemaVersion: S.Literal(
    MutaliskKhalaDelegationRunProjectionSchemaVersion,
  ),
  runRef: S.String,
  jobRef: S.String,
  environmentId: S.Literal(KhalaCodeDelegationGepaEnvironmentId),
  runner: S.Literal('mutalisk'),
  latestStage: MutaliskKhalaDelegationStage,
  decisionGrade: S.Literal(false),
  inProgress: S.Boolean,
  demandKind: S.Literal(MutaliskKhalaDelegationDemandKind),
  demandSource: S.Literal(MutaliskKhalaDelegationDemandSource),
  signature: S.Literal('khala.fleet.delegation'),
  baseModuleRef: S.String,
  seedCandidateRef: S.String,
  datasetRef: S.String,
  trainSplitRefs: S.Array(S.String),
  validationSplitRefs: S.Array(S.String),
  feedbackSchemaRef: S.Literal(
    'openagents.khala.delegation_gepa_feedback.v0',
  ),
  candidateManifestSchemaVersion: S.String,
  maxMetricCalls: S.Number,
  ownerApprovalRef: S.String,
  publicSafetyPolicyRef: S.String,
  candidateManifestRef: S.optionalKey(S.String),
  candidateRef: S.optionalKey(S.String),
  metricValueBps: S.optionalKey(S.Number),
  admissionDecision: S.optionalKey(S.Literals(['blocked', 'gated_proposal_ready'])),
  actionSubmissionProposalRef: S.optionalKey(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  progress: S.Array(MutaliskKhalaDelegationRunProgress),
  createdAt: S.String,
  updatedAt: S.String,
}) {}

export type MutaliskKhalaDelegationWorkflowStore = Readonly<{
  appendProgress: (
    progress: MutaliskKhalaDelegationRunProgress,
  ) => Effect.Effect<void>
  createRun: (
    job: MutaliskKhalaDelegationJob,
    initialProgress: MutaliskKhalaDelegationRunProgress,
  ) => Effect.Effect<void>
  getJob: (
    runRef: string,
  ) => Effect.Effect<MutaliskKhalaDelegationJob | undefined>
  getRunProjection: (
    runRef: string,
  ) => Effect.Effect<MutaliskKhalaDelegationRunProjection | undefined>
  listRunProjections: () => Effect.Effect<
    ReadonlyArray<MutaliskKhalaDelegationRunProjection>
  >
  saveBridgeOutput: (
    output: MutaliskKhalaDelegationBridgeOutput,
  ) => Effect.Effect<void>
}>

type JobRow = Readonly<{
  created_at: string
  job_json: string
  projection_json: string
}>

type ProjectionRow = Readonly<{
  projection_json: string
}>

const stageOrder: Record<MutaliskKhalaDelegationStage, number> = {
  queued: 0,
  dataset_resolved: 1,
  feedback_resolved: 2,
  optimizing: 3,
  candidate_emitted: 4,
  summary_ingested: 5,
  admission_projected: 6,
  blocked: 7,
  completed: 8,
  failed: 9,
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const sortProgress = (
  progress: ReadonlyArray<MutaliskKhalaDelegationRunProgress>,
): ReadonlyArray<MutaliskKhalaDelegationRunProgress> =>
  [...progress].sort((a, b) => {
    const order = stageOrder[a.stage] - stageOrder[b.stage]
    if (order !== 0) {
      return order
    }
    return a.updatedAt < b.updatedAt ? -1 : a.updatedAt > b.updatedAt ? 1 : 0
  })

const latestProgress = (
  progress: ReadonlyArray<MutaliskKhalaDelegationRunProgress>,
): MutaliskKhalaDelegationRunProgress | undefined =>
  sortProgress(progress).at(-1)

const optionalActionSubmissionProposalRef = (
  output: MutaliskKhalaDelegationBridgeOutput | undefined,
  latest: MutaliskKhalaDelegationRunProgress | undefined,
): string | undefined =>
  output?.actionSubmissionProposalRef ??
  latest?.actionSubmissionProposalRef ??
  undefined

const projectionFrom = (input: {
  createdAt?: string
  job: MutaliskKhalaDelegationJob
  output?: MutaliskKhalaDelegationBridgeOutput
  previous?: MutaliskKhalaDelegationRunProjection
  progress: ReadonlyArray<MutaliskKhalaDelegationRunProgress>
  summary?: MutaliskKhalaDelegationSummary
}): MutaliskKhalaDelegationRunProjection => {
  const progress = sortProgress(input.progress)
  const latest = latestProgress(progress)
  const createdAt =
    input.createdAt ?? progress[0]?.updatedAt ?? 'time.gym.operator_supplied'
  const updatedAt = latest?.updatedAt ?? createdAt
  const actionSubmissionProposalRef =
    optionalActionSubmissionProposalRef(input.output, latest)
  const projection = new MutaliskKhalaDelegationRunProjection({
    schemaVersion: MutaliskKhalaDelegationRunProjectionSchemaVersion,
    runRef: input.job.runRef,
    jobRef: input.job.jobRef,
    environmentId: KhalaCodeDelegationGepaEnvironmentId,
    runner: 'mutalisk',
    latestStage: latest?.stage ?? 'queued',
    decisionGrade: false,
    inProgress: latest?.inProgress ?? true,
    demandKind: MutaliskKhalaDelegationDemandKind,
    demandSource: MutaliskKhalaDelegationDemandSource,
    signature: input.job.signature,
    baseModuleRef: input.job.baseModuleRef,
    seedCandidateRef: input.job.seedCandidateRef,
    datasetRef: input.job.datasetRef,
    trainSplitRefs: input.job.trainSplitRefs,
    validationSplitRefs: input.job.validationSplitRefs,
    feedbackSchemaRef: input.job.feedbackSchemaRef,
    candidateManifestSchemaVersion: input.job.candidateManifestSchemaVersion,
    maxMetricCalls: input.job.maxMetricCalls,
    ownerApprovalRef: input.job.ownerApprovalRef,
    publicSafetyPolicyRef: input.job.publicSafetyPolicyRef,
    ...(input.summary?.candidateManifestRef !== undefined
      ? { candidateManifestRef: input.summary.candidateManifestRef }
      : latest?.candidateManifestRef === undefined
        ? input.previous?.candidateManifestRef === undefined
          ? {}
          : { candidateManifestRef: input.previous.candidateManifestRef }
        : { candidateManifestRef: latest.candidateManifestRef }),
    ...(input.summary?.candidateRef !== undefined
      ? { candidateRef: input.summary.candidateRef }
      : latest?.candidateRef === undefined
        ? input.previous?.candidateRef === undefined
          ? {}
          : { candidateRef: input.previous.candidateRef }
        : { candidateRef: latest.candidateRef }),
    ...(input.summary?.metricValueBps !== undefined
      ? { metricValueBps: input.summary.metricValueBps }
      : latest?.metricValueBps === undefined
        ? input.previous?.metricValueBps === undefined
          ? {}
          : { metricValueBps: input.previous.metricValueBps }
        : { metricValueBps: latest.metricValueBps }),
    ...(input.output?.admissionDecision !== undefined
      ? { admissionDecision: input.output.admissionDecision }
      : latest?.admissionDecision === undefined
        ? input.previous?.admissionDecision === undefined
          ? {}
          : { admissionDecision: input.previous.admissionDecision }
        : { admissionDecision: latest.admissionDecision }),
    ...(actionSubmissionProposalRef === undefined
      ? input.previous?.actionSubmissionProposalRef === undefined
        ? {}
        : { actionSubmissionProposalRef: input.previous.actionSubmissionProposalRef }
      : { actionSubmissionProposalRef }),
    blockerRefs: uniqueRefs([
      ...(input.output?.blockerRefs ?? []),
      ...(input.summary?.blockerRefs ?? []),
      ...(latest?.blockerRefs ?? []),
      ...(input.previous?.blockerRefs ?? []),
    ]),
    caveatRefs: uniqueRefs([
      'caveat.gym.khala_delegation_gepa.no_live_promotion',
      'caveat.gym.khala_delegation_gepa.decision_grade_false_until_live_evidence',
      ...(latest?.caveatRefs ?? []),
      ...(input.previous?.caveatRefs ?? []),
    ]),
    progress,
    createdAt,
    updatedAt,
  })
  assertMutaliskKhalaDelegationPublicSafeValues(
    'Mutalisk Khala delegation run projection',
    projection,
  )
  return projection
}

export const projectMutaliskKhalaDelegationRun = projectionFrom

const parseJob = (json: string): MutaliskKhalaDelegationJob | undefined => {
  try {
    return parseJsonWithSchema(MutaliskKhalaDelegationJob, json)
  } catch {
    return undefined
  }
}

const parseProjection = (
  json: string,
): MutaliskKhalaDelegationRunProjection | undefined => {
  try {
    return parseJsonWithSchema(MutaliskKhalaDelegationRunProjection, json)
  } catch {
    return undefined
  }
}

export const createInMemoryMutaliskKhalaDelegationWorkflowStore =
  (): MutaliskKhalaDelegationWorkflowStore & {
    snapshot: () => ReadonlyArray<MutaliskKhalaDelegationRunProjection>
  } => {
    const jobs = new Map<string, MutaliskKhalaDelegationJob>()
    const createdAtByRun = new Map<string, string>()
    const progress = new Map<string, Map<string, MutaliskKhalaDelegationRunProgress>>()
    const summaries = new Map<string, MutaliskKhalaDelegationSummary>()
    const projections = new Map<string, MutaliskKhalaDelegationRunProjection>()

    const rewriteProjection = (
      runRef: string,
      output?: MutaliskKhalaDelegationBridgeOutput,
    ) => {
      const job = jobs.get(runRef)
      if (job === undefined) {
        return
      }
      const summary = summaries.get(runRef)
      const runProgress = [...(progress.get(runRef)?.values() ?? [])]
      const createdAt = createdAtByRun.get(runRef)
      const projection = projectionFrom({
        job,
        progress: runProgress,
        ...(createdAt === undefined ? {} : { createdAt }),
        ...(output === undefined ? {} : { output }),
        ...(summary === undefined ? {} : { summary }),
      })
      projections.set(runRef, projection)
    }

    return {
      appendProgress: snapshot =>
        Effect.sync(() => {
          const stages = progress.get(snapshot.runRef) ?? new Map()
          stages.set(snapshot.stage, snapshot)
          progress.set(snapshot.runRef, stages)
          rewriteProjection(snapshot.runRef)
        }),
      createRun: (job, initialProgress) =>
        Effect.sync(() => {
          jobs.set(job.runRef, job)
          createdAtByRun.set(job.runRef, initialProgress.updatedAt)
          progress.set(
            job.runRef,
            new Map([[initialProgress.stage, initialProgress]]),
          )
          rewriteProjection(job.runRef)
        }),
      getJob: runRef => Effect.succeed(jobs.get(runRef)),
      getRunProjection: runRef => Effect.succeed(projections.get(runRef)),
      listRunProjections: () =>
        Effect.succeed(
          [...projections.values()].sort((a, b) =>
            a.updatedAt > b.updatedAt
              ? -1
              : a.updatedAt < b.updatedAt
                ? 1
                : a.runRef < b.runRef
                  ? -1
                  : 1,
          ),
        ),
      saveBridgeOutput: output =>
        Effect.sync(() => {
          jobs.set(output.job.runRef, output.job)
          summaries.set(output.job.runRef, output.summary)
          const stages = progress.get(output.job.runRef) ?? new Map()
          output.progress.forEach(snapshot => {
            stages.set(snapshot.stage, snapshot)
          })
          progress.set(output.job.runRef, stages)
          if (!createdAtByRun.has(output.job.runRef)) {
            createdAtByRun.set(
              output.job.runRef,
              output.progress[0]?.updatedAt ?? 'time.gym.operator_supplied',
            )
          }
          rewriteProjection(output.job.runRef, output)
        }),
      snapshot: () => [...projections.values()],
    }
  }

const readJobRow = async (
  db: D1Database,
  runRef: string,
): Promise<JobRow | undefined> => {
  const row = await db
    .prepare(
      `SELECT job_json, projection_json, created_at
        FROM gym_mutalisk_khala_delegation_jobs
        WHERE run_ref = ?`,
    )
    .bind(runRef)
    .first<JobRow>()
  return row ?? undefined
}

const upsertJobProjection = async (
  db: D1Database,
  job: MutaliskKhalaDelegationJob,
  projection: MutaliskKhalaDelegationRunProjection,
): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO gym_mutalisk_khala_delegation_jobs (
        run_ref,
        job_ref,
        job_json,
        projection_json,
        latest_stage,
        updated_at,
        created_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(run_ref) DO UPDATE SET
        job_ref = excluded.job_ref,
        job_json = excluded.job_json,
        projection_json = excluded.projection_json,
        latest_stage = excluded.latest_stage,
        updated_at = excluded.updated_at`,
    )
    .bind(
      job.runRef,
      job.jobRef,
      JSON.stringify(job),
      JSON.stringify(projection),
      projection.latestStage,
      projection.updatedAt,
      projection.createdAt,
    )
    .run()
}

const upsertProgress = async (
  db: D1Database,
  progress: MutaliskKhalaDelegationRunProgress,
): Promise<void> => {
  await db
    .prepare(
      `INSERT INTO gym_mutalisk_khala_delegation_progress (
        run_ref,
        stage,
        progress_json,
        updated_at
      ) VALUES (?, ?, ?, ?)
      ON CONFLICT(run_ref, stage) DO UPDATE SET
        progress_json = excluded.progress_json,
        updated_at = excluded.updated_at`,
    )
    .bind(
      progress.runRef,
      progress.stage,
      JSON.stringify(progress),
      progress.updatedAt,
    )
    .run()
}

export const makeD1MutaliskKhalaDelegationWorkflowStore = (
  db: D1Database,
): MutaliskKhalaDelegationWorkflowStore => ({
  appendProgress: progress =>
    Effect.promise(async () => {
      const row = await readJobRow(db, progress.runRef)
      if (row === undefined) {
        return
      }
      const job = parseJob(row.job_json)
      const currentProjection = parseProjection(row.projection_json)
      if (job === undefined || currentProjection === undefined) {
        return
      }
      const nextProgress = [
        ...currentProjection.progress.filter(
          snapshot => snapshot.stage !== progress.stage,
        ),
        progress,
      ]
      const projection = projectionFrom({
        createdAt: row.created_at,
        job,
        previous: currentProjection,
        progress: nextProgress,
      })
      await upsertProgress(db, progress)
      await upsertJobProjection(db, job, projection)
    }),
  createRun: (job, initialProgress) =>
    Effect.promise(async () => {
      const projection = projectionFrom({
        createdAt: initialProgress.updatedAt,
        job,
        progress: [initialProgress],
      })
      await upsertJobProjection(db, job, projection)
      await upsertProgress(db, initialProgress)
    }),
  getJob: runRef =>
    Effect.promise(async () => {
      const row = await readJobRow(db, runRef)
      return row === undefined ? undefined : parseJob(row.job_json)
    }),
  getRunProjection: runRef =>
    Effect.promise(async () => {
      const row = await readJobRow(db, runRef)
      return row === undefined ? undefined : parseProjection(row.projection_json)
    }),
  listRunProjections: () =>
    Effect.promise(async () => {
      const rows = await db
        .prepare(
          `SELECT projection_json
            FROM gym_mutalisk_khala_delegation_jobs
            ORDER BY updated_at DESC, run_ref ASC`,
        )
        .all<ProjectionRow>()
      return (rows.results ?? []).flatMap(row => {
        const parsed = parseProjection(row.projection_json)
        return parsed === undefined ? [] : [parsed]
      })
    }),
  saveBridgeOutput: output =>
    Effect.promise(async () => {
      const existing = await readJobRow(db, output.job.runRef)
      const projection = projectionFrom({
        job: output.job,
        output,
        progress: output.progress,
        summary: output.summary,
        ...(existing?.created_at === undefined
          ? {}
          : { createdAt: existing.created_at }),
      })
      await upsertJobProjection(db, output.job, projection)
      await db
        .prepare(
          `INSERT INTO gym_mutalisk_khala_delegation_summaries (
            run_ref,
            candidate_manifest_ref,
            candidate_ref,
            summary_json,
            admission_json,
            bridge_output_json,
            metric_value_bps,
            admission_decision,
            ingested_at,
            updated_at
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          ON CONFLICT(run_ref) DO UPDATE SET
            candidate_manifest_ref = excluded.candidate_manifest_ref,
            candidate_ref = excluded.candidate_ref,
            summary_json = excluded.summary_json,
            admission_json = excluded.admission_json,
            bridge_output_json = excluded.bridge_output_json,
            metric_value_bps = excluded.metric_value_bps,
            admission_decision = excluded.admission_decision,
            ingested_at = excluded.ingested_at,
            updated_at = excluded.updated_at`,
        )
        .bind(
          output.job.runRef,
          output.candidateManifestRef,
          output.candidateRef,
          JSON.stringify(output.summary),
          JSON.stringify(output.admission),
          JSON.stringify(output),
          output.metricValueBps,
          output.admissionDecision,
          projection.updatedAt,
          projection.updatedAt,
        )
        .run()
      await Promise.all(output.progress.map(snapshot => upsertProgress(db, snapshot)))
    }),
})
