// Worker API for the durable Khala Code delegation GEPA Gym workflow (#7799).
//
// Operator routes create a durable run, accept coarse progress from a local
// Mutalisk runner, and ingest the final public-safe manifest summary. The public
// route serves only the compact projection Khala Code needs for polling. The
// Worker never imports or executes Mutalisk/Python/DSPy/GEPA runtime code.
import { readRequestJsonEffect } from '@openagentsinc/effect-boundary'
import { Effect, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../../http/responses'
import { liveAtReadStaleness } from '../../public-projection-staleness'
import { currentIsoTimestamp } from '../../runtime-primitives'
import {
  assertMutaliskKhalaDelegationPublicSafeValues,
  buildMutaliskKhalaDelegationRunProgress,
  createMutaliskKhalaDelegationJob,
  MutaliskKhalaDelegationGymBridgeUnsafe,
  MutaliskKhalaDelegationStage,
  runMutaliskKhalaDelegationNoUiBridge,
} from './mutalisk-khala-delegation-bridge'
import {
  MutaliskKhalaDelegationRunProjectionSchemaVersion,
  type MutaliskKhalaDelegationWorkflowStore,
  type MutaliskKhalaDelegationRunProjection,
} from './mutalisk-khala-delegation-store'

export const MutaliskKhalaDelegationRunsEnvelopeSchemaVersion =
  'openagents.gym.mutalisk_khala_delegation_runs.v0'

const CreateRunBody = S.Struct({
  baseModuleRef: S.optionalKey(S.String),
  datasetRef: S.optionalKey(S.String),
  jobRef: S.optionalKey(S.String),
  maxMetricCalls: S.optionalKey(S.Number),
  ownerApprovalRef: S.optionalKey(S.String),
  publicSafetyPolicyRef: S.optionalKey(S.String),
  refSeed: S.optionalKey(S.String),
  runRef: S.optionalKey(S.String),
  seedCandidateRef: S.optionalKey(S.String),
  trainSplitRefs: S.optionalKey(S.Array(S.String)),
  validationSplitRefs: S.optionalKey(S.Array(S.String)),
})

const ProgressBody = S.Struct({
  actionSubmissionProposalRef: S.optionalKey(S.String),
  admissionDecision: S.optionalKey(S.Literals(['blocked', 'gated_proposal_ready'])),
  blockerRefs: S.optionalKey(S.Array(S.String)),
  candidateManifestRef: S.optionalKey(S.String),
  candidateRef: S.optionalKey(S.String),
  caveatRefs: S.optionalKey(S.Array(S.String)),
  metricValueBps: S.optionalKey(S.Number),
  runRef: S.String,
  stage: MutaliskKhalaDelegationStage,
  updatedAt: S.optionalKey(S.String),
})

const SummaryIngestBody = S.Struct({
  actorRef: S.optionalKey(S.String),
  artifactRefs: S.optionalKey(S.Array(S.String)),
  manifestSummary: S.Record(S.String, S.Unknown),
  observedAt: S.optionalKey(S.String),
  optimizerRunRefs: S.optionalKey(S.Array(S.String)),
  runRef: S.String,
})

const staleness = () =>
  liveAtReadStaleness([
    'gym.mutalisk_khala_delegation.run_created',
    'gym.mutalisk_khala_delegation.progress_ingested',
    'gym.mutalisk_khala_delegation.summary_ingested',
  ])

export type MutaliskKhalaDelegationRouteInput = Readonly<{
  listRunProjections?: () => ReadonlyArray<MutaliskKhalaDelegationRunProjection>
  nowIso?: () => string
  store?: Pick<
    MutaliskKhalaDelegationWorkflowStore,
    'getRunProjection' | 'listRunProjections'
  >
}>

export type MutaliskKhalaDelegationOperatorRouteInput =
  MutaliskKhalaDelegationRouteInput &
    Readonly<{
      makeRunToken?: () => string
      requireAdminApiToken: (request: Request) => Promise<boolean>
      store?: MutaliskKhalaDelegationWorkflowStore
    }>

const run = <A>(effect: Effect.Effect<A>): Promise<A> => Effect.runPromise(effect)

const unauthorized = () =>
  noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })

const unavailable = () =>
  noStoreJsonResponse(
    {
      error: 'mutalisk_khala_delegation_unavailable',
      reason: 'No writable Mutalisk Khala delegation workflow store is configured.',
    },
    { status: 503 },
  )

const notFound = (runRef: string) =>
  noStoreJsonResponse(
    { error: 'mutalisk_khala_delegation_run_not_found', runRef },
    { status: 404 },
  )

const badRequest = (reason: string) =>
  noStoreJsonResponse(
    { error: 'mutalisk_khala_delegation_rejected', reason },
    { status: 400 },
  )

const reasonFor = (error: unknown): string => {
  if (error instanceof MutaliskKhalaDelegationGymBridgeUnsafe) {
    return error.reason
  }
  if (
    typeof error === 'object' &&
    error !== null &&
    'reasonRef' in error &&
    typeof error.reasonRef === 'string'
  ) {
    return error.reasonRef
  }
  return error instanceof Error ? error.message : String(error)
}

const defaultRunToken = (): string =>
  crypto.randomUUID().replaceAll('-', '').slice(0, 24)

const listProjections = (
  input: MutaliskKhalaDelegationRouteInput,
): Effect.Effect<ReadonlyArray<MutaliskKhalaDelegationRunProjection>> => {
  if (input.store !== undefined) {
    return input.store.listRunProjections()
  }
  return Effect.succeed((input.listRunProjections ?? (() => []))())
}

const projectionsEnvelope = (
  scope: 'operator' | 'public',
  input: MutaliskKhalaDelegationRouteInput,
  runs: ReadonlyArray<MutaliskKhalaDelegationRunProjection>,
) => ({
  schemaVersion: MutaliskKhalaDelegationRunsEnvelopeSchemaVersion,
  runSchemaVersion: MutaliskKhalaDelegationRunProjectionSchemaVersion,
  scope,
  generatedAt: (input.nowIso ?? currentIsoTimestamp)(),
  staleness: staleness(),
  runs,
})

const requireOperator = (
  request: Request,
  input: MutaliskKhalaDelegationOperatorRouteInput,
): Effect.Effect<boolean> =>
  Effect.promise(() => input.requireAdminApiToken(request))

const operatorGuard = (
  request: Request,
  input: MutaliskKhalaDelegationOperatorRouteInput,
  onAuthorized: () => Effect.Effect<Response>,
): Effect.Effect<Response> =>
  requireOperator(request, input).pipe(
    Effect.flatMap(authorized =>
      authorized ? onAuthorized() : Effect.succeed(unauthorized()),
    ),
  )

const handleCreateRun = (
  request: Request,
  input: MutaliskKhalaDelegationOperatorRouteInput,
): Effect.Effect<Response> => {
  const store = input.store
  if (store === undefined) {
    return Effect.succeed(unavailable())
  }
  return readRequestJsonEffect(
    CreateRunBody,
    request,
    'gym.mutalisk_khala_delegation.create_run.body',
  ).pipe(
    Effect.flatMap(body =>
      Effect.try({
        catch: reasonFor,
        try: () => {
          assertMutaliskKhalaDelegationPublicSafeValues(
            'Mutalisk Khala delegation create-run body',
            body,
          )
          const token = body.refSeed ?? (input.makeRunToken ?? defaultRunToken)()
          return createMutaliskKhalaDelegationJob({
            ...body,
            refSeed: token,
          })
        },
      }),
    ),
    Effect.flatMap(job => {
      const nowIso = (input.nowIso ?? currentIsoTimestamp)()
      const progress = buildMutaliskKhalaDelegationRunProgress({
        job,
        observedAt: nowIso,
        stage: 'queued',
      })
      return store.createRun(job, progress).pipe(
        Effect.flatMap(() => store.getRunProjection(job.runRef)),
        Effect.map(projection =>
          noStoreJsonResponse(
            {
              schemaVersion: MutaliskKhalaDelegationRunsEnvelopeSchemaVersion,
              kind: 'mutalisk_khala_delegation_run_created',
              runRef: job.runRef,
              jobRef: job.jobRef,
              run: projection,
            },
            { status: 201 },
          ),
        ),
      )
    }),
    Effect.catch(error => Effect.succeed(badRequest(reasonFor(error)))),
  )
}

const handleAppendProgress = (
  request: Request,
  input: MutaliskKhalaDelegationOperatorRouteInput,
): Effect.Effect<Response> => {
  const store = input.store
  if (store === undefined) {
    return Effect.succeed(unavailable())
  }
  return readRequestJsonEffect(
    ProgressBody,
    request,
    'gym.mutalisk_khala_delegation.progress.body',
  ).pipe(
    Effect.flatMap(body =>
      Effect.try({
        catch: reasonFor,
        try: () => {
          assertMutaliskKhalaDelegationPublicSafeValues(
            'Mutalisk Khala delegation progress body',
            body,
          )
          return body
        },
      }),
    ),
    Effect.flatMap(body =>
      store.getJob(body.runRef).pipe(
        Effect.flatMap(job => {
          if (job === undefined) {
            return Effect.succeed(notFound(body.runRef))
          }
          const progress = buildMutaliskKhalaDelegationRunProgress({
            blockerRefs: body.blockerRefs ?? [],
            caveatRefs: body.caveatRefs ?? [],
            job,
            observedAt: body.updatedAt ?? (input.nowIso ?? currentIsoTimestamp)(),
            stage: body.stage,
            ...(body.actionSubmissionProposalRef === undefined
              ? {}
              : { actionSubmissionProposalRef: body.actionSubmissionProposalRef }),
            ...(body.admissionDecision === undefined
              ? {}
              : { admissionDecision: body.admissionDecision }),
            ...(body.candidateManifestRef === undefined
              ? {}
              : { candidateManifestRef: body.candidateManifestRef }),
            ...(body.candidateRef === undefined
              ? {}
              : { candidateRef: body.candidateRef }),
            ...(body.metricValueBps === undefined
              ? {}
              : { metricValueBps: body.metricValueBps }),
          })
          return store.appendProgress(progress).pipe(
            Effect.flatMap(() => store.getRunProjection(job.runRef)),
            Effect.map(projection =>
              noStoreJsonResponse(
                {
                  schemaVersion: MutaliskKhalaDelegationRunsEnvelopeSchemaVersion,
                  kind: 'mutalisk_khala_delegation_progress_ingested',
                  runRef: job.runRef,
                  run: projection,
                },
                { status: 201 },
              ),
            ),
          )
        }),
      ),
    ),
    Effect.catch(error => Effect.succeed(badRequest(reasonFor(error)))),
  )
}

const handleIngestSummary = (
  request: Request,
  input: MutaliskKhalaDelegationOperatorRouteInput,
): Effect.Effect<Response> => {
  const store = input.store
  if (store === undefined) {
    return Effect.succeed(unavailable())
  }
  return readRequestJsonEffect(
    SummaryIngestBody,
    request,
    'gym.mutalisk_khala_delegation.summary.body',
  ).pipe(
    Effect.flatMap(body =>
      Effect.try({
        catch: reasonFor,
        try: () => {
          assertMutaliskKhalaDelegationPublicSafeValues(
            'Mutalisk Khala delegation summary body',
            body,
          )
          return body
        },
      }),
    ),
    Effect.flatMap(body =>
      store.getJob(body.runRef).pipe(
        Effect.flatMap(job => {
          if (job === undefined) {
            return Effect.succeed(notFound(body.runRef))
          }
          return Effect.try({
            catch: reasonFor,
            try: () =>
              runMutaliskKhalaDelegationNoUiBridge(body.manifestSummary, {
                job,
                observedAt:
                  body.observedAt ?? (input.nowIso ?? currentIsoTimestamp)(),
                ...(body.actorRef === undefined
                  ? {}
                  : { actorRef: body.actorRef }),
                ...(body.artifactRefs === undefined
                  ? {}
                  : { artifactRefs: body.artifactRefs }),
                ...(body.optimizerRunRefs === undefined
                  ? {}
                  : { optimizerRunRefs: body.optimizerRunRefs }),
              }),
          }).pipe(
            Effect.flatMap(output =>
              store.saveBridgeOutput(output).pipe(
                Effect.flatMap(() => store.getRunProjection(output.job.runRef)),
                Effect.map(projection =>
                  noStoreJsonResponse(
                    {
                      schemaVersion:
                        MutaliskKhalaDelegationRunsEnvelopeSchemaVersion,
                      kind: 'mutalisk_khala_delegation_summary_ingested',
                      runRef: output.job.runRef,
                      candidateManifestRef: output.candidateManifestRef,
                      candidateRef: output.candidateRef,
                      metricValueBps: output.metricValueBps,
                      admissionDecision: output.admissionDecision,
                      actionSubmissionProposalRef:
                        output.actionSubmissionProposalRef,
                      decisionGrade: output.decisionGrade,
                      run: projection,
                    },
                    { status: 201 },
                  ),
                ),
              ),
            ),
          )
        }),
      ),
    ),
    Effect.catch(error => Effect.succeed(badRequest(reasonFor(error)))),
  )
}

export const handleOperatorMutaliskKhalaDelegationRunsApi = (
  request: Request,
  input: MutaliskKhalaDelegationOperatorRouteInput,
): Effect.Effect<Response> => {
  if (request.method !== 'GET' && request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['GET', 'POST']))
  }
  return operatorGuard(request, input, () => {
    if (request.method === 'POST') {
      return handleCreateRun(request, input)
    }
    return listProjections(input).pipe(
      Effect.map(runs =>
        noStoreJsonResponse(projectionsEnvelope('operator', input, runs)),
      ),
    )
  })
}

export const handleOperatorMutaliskKhalaDelegationProgressApi = (
  request: Request,
  input: MutaliskKhalaDelegationOperatorRouteInput,
): Effect.Effect<Response> => {
  if (request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['POST']))
  }
  return operatorGuard(request, input, () => handleAppendProgress(request, input))
}

export const handleOperatorMutaliskKhalaDelegationSummaryApi = (
  request: Request,
  input: MutaliskKhalaDelegationOperatorRouteInput,
): Effect.Effect<Response> => {
  if (request.method !== 'POST') {
    return Effect.succeed(methodNotAllowed(['POST']))
  }
  return operatorGuard(request, input, () => handleIngestSummary(request, input))
}

export const handlePublicMutaliskKhalaDelegationRunsApi = (
  request: Request,
  input: MutaliskKhalaDelegationRouteInput = {},
): Effect.Effect<Response> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  return listProjections(input).pipe(
    Effect.map(runs =>
      noStoreJsonResponse(projectionsEnvelope('public', input, runs)),
    ),
  )
}

export const runMutaliskKhalaDelegationRouteEffect = run
