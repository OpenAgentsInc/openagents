import { Effect, Match as M, Schema as S } from 'effect'

import { publicCs336A5EvalProjection } from './cs336-a5-alignment-homework'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import {
  TrainingLeaderboardLanes,
  buildTrainingLeaderboardsProjection,
  settledSatsFromPaymentAuthorityReceipt,
} from './training-leaderboards'
import {
  Cs336A2DeviceBenchmarkEvidenceRequest,
  admitCs336A2DeviceBenchmarkEvidence,
  publicDeviceCapabilityProjection,
} from './training-device-capability'
import {
  type TrainingAuthorityStore,
  TrainingAuthorityStoreError,
  TrainingRunPlanRequest,
  type TrainingRunState,
  TrainingRunTransitionRequest,
  TrainingWindowLeaseClaimRequest,
  TrainingWindowPlanRequest,
  type TrainingWindowState,
  TrainingWindowTransitionRequest,
  buildTrainingRunRecord,
  buildTrainingWindowLeaseRecord,
  buildTrainingWindowRecord,
  publicTrainingRunProjection,
  publicTrainingRunSummary,
  publicTrainingWindowProjection,
  selectTrainingLeaseCandidate,
  trainingAuthorityStoreErrorFromUnknown,
  transitionTrainingRunRecord,
  transitionTrainingWindowRecord,
} from './training-run-window-authority'
import {
  Cs336A3ScalingSweepEvidenceRequest,
  admitCs336A3ScalingSweepEvidence,
  publicScalingSweepProjection,
} from './training-scaling-sweep'
import {
  TrainingWindowBootstrapGrantRequest,
  decideTrainingWindowBootstrapGrant,
} from './training-window-bootstrap'
import {
  Cs336A1RealGradientEvidenceRequest,
  admitCs336A1RealGradientEvidence,
} from './training-real-gradient-evidence'
import {
  Cs336A4DataRefineryEvidenceRequest,
  Cs336A4RequiredVerifiedStageCount,
  admitCs336A4DataRefineryEvidence,
  publicDataRefineryProjection,
} from './training-data-refinery'
import {
  Cs336A5AlignmentEvidenceRequest,
  admitCs336A5AlignmentEvidence,
} from './training-alignment-evals'
import {
  TrainingRunAdmissionRequest,
  decideTassadarRunAdmission,
} from './tassadar-run-admission'

type HttpResponse = globalThis.Response

type TrainingLeaderboardSettlementReceiptReader = Readonly<{
  readPaymentAuthorityReceiptByRef: (
    receiptRef: string,
  ) => Promise<
    | Readonly<{ publicProjectionJson: string; receiptKind: string }>
    | undefined
  >
}>

type TrainingRunWindowRouteDependencies<Bindings> = Readonly<{
  makeId?: () => string
  makePayoutLedgerStore?: (
    env: Bindings,
  ) => TrainingLeaderboardSettlementReceiptReader
  makeStore: (env: Bindings) => TrainingAuthorityStore
  nowIso?: () => string
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
}>

type TrainingRunWindowRouteEnv = Readonly<Record<string, unknown>>

class TrainingRunWindowUnauthorized extends S.TaggedErrorClass<TrainingRunWindowUnauthorized>()(
  'TrainingRunWindowUnauthorized',
  {},
) {}

type TrainingRunWindowRouteError =
  | TrainingAuthorityStoreError
  | TrainingRunWindowUnauthorized

const routeErrorResponse = (error: TrainingRunWindowRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      TrainingAuthorityStoreError: storeError =>
        noStoreJsonResponse(
          {
            error: `training_authority_${storeError.kind}`,
            reason: storeError.reason,
          },
          {
            status:
              storeError.kind === 'conflict'
                ? 409
                : storeError.kind === 'forbidden'
                  ? 403
                  : storeError.kind === 'not_found'
                    ? 404
                    : storeError.kind === 'storage_error'
                      ? 500
                      : 400,
          },
        ),
      TrainingRunWindowUnauthorized: () => unauthorized(),
    }),
    M.exhaustive,
  )

const decodeBody = <A>(
  request: Request,
  schema: S.Decoder<A>,
): Effect.Effect<A, TrainingAuthorityStoreError> =>
  Effect.tryPromise({
    catch: error =>
      new TrainingAuthorityStoreError({
        kind: 'validation_error',
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: async () =>
      decodeUnknownWithSchema(schema, await readJsonObject(request)),
  })

const routeNowIso = <Bindings>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
): string => dependencies.nowIso?.() ?? currentIsoTimestamp()

const routeMakeId = <Bindings>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
): string => (dependencies.makeId ?? randomUuid)()

const requireAdmin = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<void, TrainingRunWindowUnauthorized> =>
  Effect.tryPromise({
    catch: () => new TrainingRunWindowUnauthorized({}),
    try: () =>
      dependencies.requireAdminApiToken?.(request, env) ??
      Promise.resolve(false),
  }).pipe(
    Effect.flatMap(isAdmin =>
      isAdmin
        ? Effect.void
        : Effect.fail(new TrainingRunWindowUnauthorized({})),
    ),
  )

const routePlanRun = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(request, TrainingRunPlanRequest)
    const nowIso = routeNowIso(dependencies)
    const record = buildTrainingRunRecord({
      makeId: () => routeMakeId(dependencies),
      nowIso,
      request: body,
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => dependencies.makeStore(env).planRun(record),
    })

    return noStoreJsonResponse({
      run: publicTrainingRunProjection(stored, nowIso),
    })
  })

const routePlanWindow = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(request, TrainingWindowPlanRequest)
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const run = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.readRun(body.trainingRunRef),
    })

    if (run === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found.',
      })
    }

    const record = buildTrainingWindowRecord({
      makeId: () => routeMakeId(dependencies),
      nowIso,
      request: body,
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.planWindow(record),
    })

    return noStoreJsonResponse({
      window: publicTrainingWindowProjection(stored, nowIso),
    })
  })

const routeTransitionWindow = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  windowRef: string,
  transitionKind: string,
  nextState: TrainingWindowState,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(request, TrainingWindowTransitionRequest)
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const current = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.readWindow(windowRef),
    })

    if (current === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training window not found.',
      })
    }

    const transitioned = transitionTrainingWindowRecord({
      actorRef: body.actorRef ?? 'operator.openagents.training_authority',
      eventId: routeMakeId(dependencies),
      nextState,
      nowIso,
      receiptRef: body.receiptRef,
      sealMetadata: body.sealMetadata,
      transitionKind,
      window: current,
    })
    const persistTransition = Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () =>
        store.transitionWindow(transitioned.window, transitioned.event),
    })
    // The seal mutation runs inside the run-level merge barrier (Pluralis
    // roadmap P1.3, issue #4851): the barrier is raised before the seal is
    // persisted and lowered once the operation finishes, so bootstrap
    // grants and join transitions queue instead of reading mid-seal state.
    // transitionWindow persists through one atomic D1 batch, so an
    // observed failure leaves no partial state and the barrier may clear;
    // an unobserved crash conservatively leaves the barrier up.
    const stored = yield* nextState === 'sealed'
      ? Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () =>
            store.beginRunSealBarrier(current.trainingRunRef, nowIso),
        }).pipe(
          Effect.andThen(persistTransition),
          Effect.ensuring(
            Effect.promise(() =>
              store
                .clearRunSealBarrier(current.trainingRunRef)
                .catch(() => undefined),
            ),
          ),
        )
      : persistTransition

    return noStoreJsonResponse({
      window: publicTrainingWindowProjection(stored, nowIso),
    })
  })

const routeTransitionRun = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
  nextState: TrainingRunState,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(request, TrainingRunTransitionRequest)
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const current = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.readRun(trainingRunRef),
    })

    if (current === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found.',
      })
    }

    const transitioned = yield* Effect.try({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () =>
        transitionTrainingRunRecord({
          nextState,
          nowIso,
          receiptRef: body.receiptRef,
          run: current,
        }),
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.transitionRun(transitioned.run),
    })

    return noStoreJsonResponse({
      run: publicTrainingRunProjection(stored, nowIso),
    })
  })

const routeAdmitRunContributor = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(request, TrainingRunAdmissionRequest)
    const store = dependencies.makeStore(env)
    const run = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.readRun(trainingRunRef),
    })

    if (run === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found.',
      })
    }

    const admission = yield* Effect.try({
      catch: error =>
        new TrainingAuthorityStoreError({
          kind: 'validation_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => decideTassadarRunAdmission(body),
    })

    return noStoreJsonResponse({ admission, trainingRunRef: run.trainingRunRef })
  })

const routeBootstrapGrant = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const body = yield* decodeBody(request, TrainingWindowBootstrapGrantRequest)
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const run = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.readRun(trainingRunRef),
    })

    if (run === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found.',
      })
    }

    const windows = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowsForRun(trainingRunRef, 100),
    })
    const outcome = decideTrainingWindowBootstrapGrant({
      joinerReceiptRefs: body.receiptRefs,
      joinerRef: body.joinerRef,
      makeId: () => routeMakeId(dependencies),
      requestedAtIso: nowIso,
      run,
      windows,
    })

    return noStoreJsonResponse({ outcome })
  })

const routeClaimLease = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const body = yield* decodeBody(request, TrainingWindowLeaseClaimRequest)
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const windows = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listClaimableWindows(nowIso, 25),
    })
    const selected = selectTrainingLeaseCandidate(windows)

    if (selected === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'No active training window is currently claimable.',
      })
    }

    const lease = buildTrainingWindowLeaseRecord({
      makeId: () => routeMakeId(dependencies),
      nowIso,
      request: body,
      window: selected,
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.claimLease(lease, nowIso),
    })

    return noStoreJsonResponse({ lease: stored })
  })

const routeReadRun = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const record = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => dependencies.makeStore(env).readRun(trainingRunRef),
    })

    if (record === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found.',
      })
    }

    const store = dependencies.makeStore(env)
    const windows = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowsForRun(trainingRunRef, 100),
    })
    const leases = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowLeasesForRun(trainingRunRef, 1000),
    })
    const challenges = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listVerificationChallengesForRun(trainingRunRef, 1000),
    })

    return noStoreJsonResponse({
      run: publicTrainingRunProjection(record, nowIso),
      summary: publicTrainingRunSummary({
        challenges,
        leases,
        nowIso,
        run: record,
        windows,
      }),
    })
  })

const routeListRuns = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const runs = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listRuns(50),
    })
    const summaries = yield* Effect.forEach(runs, run =>
      Effect.gen(function* () {
        const windows = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowsForRun(run.trainingRunRef, 100),
        })
        const leases = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowLeasesForRun(run.trainingRunRef, 1000),
        })
        const challenges = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () =>
            store.listVerificationChallengesForRun(run.trainingRunRef, 1000),
        })

        return publicTrainingRunSummary({
          challenges,
          leases,
          nowIso,
          run,
          windows,
        })
      }),
    )

    return noStoreJsonResponse({
      runs: runs.map(run => publicTrainingRunProjection(run, nowIso)),
      summaries,
    })
  })

const routeA1Leaderboard = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const runs = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listRuns(50),
    })
    const summaries = yield* Effect.forEach(runs, run =>
      Effect.gen(function* () {
        const windows = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowsForRun(run.trainingRunRef, 100),
        })
        const leases = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowLeasesForRun(run.trainingRunRef, 1000),
        })
        const challenges = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () =>
            store.listVerificationChallengesForRun(run.trainingRunRef, 1000),
        })

        return publicTrainingRunSummary({
          challenges,
          leases,
          nowIso,
          run,
          windows,
        })
      }),
    )
    const rows = summaries
      .flatMap(summary => summary.realGradient.leaderboardRows)
      .sort((left, right) => {
        if (
          left.bestValidationLoss !== null &&
          right.bestValidationLoss !== null
        ) {
          return left.bestValidationLoss - right.bestValidationLoss
        }

        if (left.bestValidationLoss !== null) {
          return -1
        }

        if (right.bestValidationLoss !== null) {
          return 1
        }

        return right.verifiedWindowCount - left.verifiedWindowCount
      })
      .map((row, index) => ({ ...row, rank: index + 1 }))

    return noStoreJsonResponse({
      leaderboardRows: rows,
      scopeBoundaryRefs: [
        'scope.cs336_a1.bounded_multi_device_training_evidence_only',
        'scope.cs336_a1.does_not_replace_qwen_finetune_gate_4670',
        'scope.cs336_a1.no_first_real_training_run_green_copy_from_this_issue_alone',
      ],
      sourceRefs: [
        'route:/api/training/leaderboards/a1',
        'route:/api/training/runs',
      ],
    })
  })

const maxSettlementReceiptLookups = 128

const resolveSettledSatsByReceiptRef = <
  Bindings extends TrainingRunWindowRouteEnv,
>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
  draft: ReturnType<typeof buildTrainingLeaderboardsProjection>,
): Effect.Effect<
  ReadonlyMap<string, number>,
  TrainingRunWindowRouteError
> =>
  Effect.gen(function* () {
    const makePayoutLedgerStore = dependencies.makePayoutLedgerStore

    if (makePayoutLedgerStore === undefined) {
      return new Map<string, number>()
    }

    const receiptRefs = [
      ...new Set(
        draft.lanes.flatMap(lane =>
          lane.rows.flatMap(row => row.receiptRefs),
        ),
      ),
    ]
      .sort()
      .slice(0, maxSettlementReceiptLookups)

    if (receiptRefs.length === 0) {
      return new Map<string, number>()
    }

    const ledger = makePayoutLedgerStore(env)
    const entries = yield* Effect.forEach(receiptRefs, receiptRef =>
      Effect.tryPromise({
        catch: trainingAuthorityStoreErrorFromUnknown,
        try: async () => {
          const record = await ledger.readPaymentAuthorityReceiptByRef(
            receiptRef,
          )

          return [
            receiptRef,
            record === undefined
              ? 0
              : settledSatsFromPaymentAuthorityReceipt(record),
          ] as const
        },
      }),
    )

    return new Map(entries.filter(([, settledSats]) => settledSats > 0))
  })

const routeTrainingLeaderboards = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
  laneFilter?: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const runs = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listRuns(50),
    })
    const inputs = yield* Effect.forEach(runs, run =>
      Effect.gen(function* () {
        const windows = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowsForRun(run.trainingRunRef, 100),
        })
        const leases = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowLeasesForRun(run.trainingRunRef, 1000),
        })
        const challenges = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () =>
            store.listVerificationChallengesForRun(run.trainingRunRef, 1000),
        })

        return {
          a2Projection: publicDeviceCapabilityProjection({
            challenges,
            leases,
            run,
            windows,
          }),
          a3Projection: publicScalingSweepProjection({
            challenges,
            leases,
            run,
            windows,
          }),
          a5Projection: publicCs336A5EvalProjection({
            challenges,
            leases,
            run,
            windows,
          }),
          summary: publicTrainingRunSummary({
            challenges,
            leases,
            nowIso,
            run,
            windows,
          }),
        }
      }),
    )
    const builderInput = {
      a2Projections: inputs.map(input => input.a2Projection),
      a3Projections: inputs.map(input => input.a3Projection),
      a5Projections: inputs.map(input => input.a5Projection),
      runs,
      summaries: inputs.map(input => input.summary),
    }
    const settledSatsByReceiptRef = yield* resolveSettledSatsByReceiptRef(
      dependencies,
      env,
      buildTrainingLeaderboardsProjection(builderInput),
    )
    const projection = buildTrainingLeaderboardsProjection({
      ...builderInput,
      settledSatsByReceiptRef,
    })
    const lanes =
      laneFilter === undefined
        ? projection.lanes
        : projection.lanes.filter(lane => lane.lane === laneFilter)

    if (laneFilter !== undefined && lanes.length === 0) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training leaderboard lane not found.',
      })
    }

    return noStoreJsonResponse({
      ...projection,
      blockerRefs: lanes.flatMap(lane => lane.blockerRefs),
      lanes,
    })
  })

const routeA3IsoFlop = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const store = dependencies.makeStore(env)
    const runs = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listRuns(50),
    })
    const projections = yield* Effect.forEach(runs, run =>
      Effect.gen(function* () {
        const windows = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowsForRun(run.trainingRunRef, 100),
        })
        const leases = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowLeasesForRun(run.trainingRunRef, 1000),
        })
        const challenges = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () =>
            store.listVerificationChallengesForRun(run.trainingRunRef, 1000),
        })

        return publicScalingSweepProjection({
          challenges,
          leases,
          run,
          windows,
        })
      }),
    )
    const cells = projections.flatMap(projection => projection.cells)
    const fitArtifacts = projections
      .map(projection => projection.fitArtifact)
      .filter(artifact => artifact !== null)

    return noStoreJsonResponse({
      blockerRefs:
        cells.filter(cell => cell.verified).length >= 20 &&
        fitArtifacts.length > 0
          ? []
          : [
              'blocker.cs336_a3.requires_twenty_verified_cells',
              'blocker.cs336_a3.operator_funding_required_for_paid_cells',
              'blocker.cs336_a3.fit_artifact_not_published',
            ],
      cells,
      fitArtifacts,
      projections,
      schemaVersion: 'openagents.training.isoflop_dashboard.v1',
      sourceRefs: [
        'route:/api/training/isoflop/a3',
        'route:/api/training/runs',
      ],
    })
  })

const routeA4DataRefinery = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const store = dependencies.makeStore(env)
    const runs = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listRuns(50),
    })
    const projections = yield* Effect.forEach(runs, run =>
      Effect.gen(function* () {
        const windows = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowsForRun(run.trainingRunRef, 100),
        })
        const leases = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowLeasesForRun(run.trainingRunRef, 1000),
        })
        const challenges = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () =>
            store.listVerificationChallengesForRun(run.trainingRunRef, 1000),
        })

        return publicDataRefineryProjection({
          challenges,
          leases,
          run,
          windows,
        })
      }),
    )
    const shards = projections.flatMap(projection => projection.shards)
    const verifiedStages = [
      ...new Set(
        shards
          .filter(shard => shard.verified)
          .map(shard => shard.stage),
      ),
    ].sort()

    return noStoreJsonResponse({
      blockerRefs:
        verifiedStages.length >= Cs336A4RequiredVerifiedStageCount
          ? []
          : [
              'blocker.cs336_a4.requires_three_verified_stages',
              'blocker.cs336_a4.operator_funding_required_for_paid_shards',
            ],
      evalDeltaBonusBlockerRefs: [
        'blocker.cs336_a4.fixed_trainer_eval_loop_required_for_quality_bonus',
        'blocker.cs336_a4.operator_funding_required_for_bonus_settlement',
        'blocker.cs336_a4.psionic_classifier_adapters_partial',
      ],
      observedVerifiedStages: verifiedStages,
      projections,
      requiredVerifiedStageCount: Cs336A4RequiredVerifiedStageCount,
      schemaVersion: 'openagents.training.data_refinery_dashboard.v1',
      shards,
      sourceRefs: [
        'route:/api/training/refinery/a4',
        'route:/api/training/runs',
      ],
    })
  })

const routeA2DeviceCapabilities = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const store = dependencies.makeStore(env)
    const runs = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listRuns(50),
    })
    const projections = yield* Effect.forEach(runs, run =>
      Effect.gen(function* () {
        const windows = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowsForRun(run.trainingRunRef, 100),
        })
        const leases = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowLeasesForRun(run.trainingRunRef, 1000),
        })
        const challenges = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () =>
            store.listVerificationChallengesForRun(run.trainingRunRef, 1000),
        })

        return publicDeviceCapabilityProjection({
          challenges,
          leases,
          run,
          windows,
        })
      }),
    )
    const classDistributions = projections.flatMap(
      projection => projection.classDistributions,
    )
    const observedDeviceClassCount = new Set(
      classDistributions.map(distribution => distribution.deviceClassRef),
    ).size
    const verifiedCount = classDistributions.filter(
      distribution => distribution.verified,
    ).length

    return noStoreJsonResponse({
      blockerRefs:
        classDistributions.length > 0 &&
        verifiedCount === classDistributions.length
          ? []
          : [
              'blocker.cs336_a2.requires_receipted_benchmark_results',
              'blocker.cs336_a2.requires_statistical_cross_check',
              'blocker.cs336_a2.requires_replication_across_same_class_devices',
            ],
      classDistributions,
      observedDeviceClassCount,
      observedMeasurementCount: classDistributions.length,
      projections,
      schemaVersion: 'openagents.training.device_capability_dashboard.v1',
      sourceRefs: [
        'route:/api/training/device-capabilities/a2',
        'route:/api/training/runs',
      ],
    })
  })

const routeA5EvalSuites = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const store = dependencies.makeStore(env)
    const runs = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listRuns(50),
    })
    const projections = yield* Effect.forEach(runs, run =>
      Effect.gen(function* () {
        const windows = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowsForRun(run.trainingRunRef, 100),
        })
        const leases = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () => store.listWindowLeasesForRun(run.trainingRunRef, 1000),
        })
        const challenges = yield* Effect.tryPromise({
          catch: trainingAuthorityStoreErrorFromUnknown,
          try: () =>
            store.listVerificationChallengesForRun(run.trainingRunRef, 1000),
        })

        return publicCs336A5EvalProjection({
          challenges,
          leases,
          run,
          windows,
        })
      }),
    )
    const evalSuites = projections.flatMap(projection => projection.evalSuites)
    const verifiedSuiteCount = evalSuites.filter(
      suite => suite.verificationRefs.length > 0,
    ).length

    return noStoreJsonResponse({
      blockerRefs:
        evalSuites.length > 0 && verifiedSuiteCount > 0
          ? []
          : [
              'blocker.cs336_a5.requires_rollout_receipts',
              'blocker.cs336_a5.requires_grading_verification',
              'blocker.cs336_a5.requires_public_eval_suite_receipt',
              'blocker.cs336_a5.policy_gradient_update_waits_on_4669',
            ],
      evalSuites,
      projections,
      schemaVersion: 'openagents.training.a5_eval_dashboard.v1',
      sourceRefs: ['route:/api/training/evals/a5', 'route:/api/training/runs'],
      updateBoundaryRef: 'issue.github.openagents.4669',
    })
  })

const routeAttachDeviceBenchmarkEvidence = <
  Bindings extends TrainingRunWindowRouteEnv,
>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(
      request,
      Cs336A2DeviceBenchmarkEvidenceRequest,
    )
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const run = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.readRun(trainingRunRef),
    })

    if (run === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found.',
      })
    }

    const admitted = yield* Effect.try({
      catch: error =>
        new TrainingAuthorityStoreError({
          kind: 'validation_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        admitCs336A2DeviceBenchmarkEvidence({ nowIso, request: body, run }),
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.attachRunEvidence(admitted),
    })
    const windows = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowsForRun(trainingRunRef, 100),
    })
    const leases = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowLeasesForRun(trainingRunRef, 1000),
    })
    const challenges = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listVerificationChallengesForRun(trainingRunRef, 1000),
    })

    return noStoreJsonResponse({
      dataset: publicDeviceCapabilityProjection({
        challenges,
        leases,
        run: stored,
        windows,
      }),
      run: publicTrainingRunProjection(stored, nowIso),
    })
  })

const routeAttachScalingSweepEvidence = <
  Bindings extends TrainingRunWindowRouteEnv,
>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(request, Cs336A3ScalingSweepEvidenceRequest)
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const run = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.readRun(trainingRunRef),
    })

    if (run === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found.',
      })
    }

    const admitted = yield* Effect.try({
      catch: error =>
        new TrainingAuthorityStoreError({
          kind: 'validation_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        admitCs336A3ScalingSweepEvidence({ nowIso, request: body, run }),
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.attachRunEvidence(admitted),
    })
    const windows = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowsForRun(trainingRunRef, 100),
    })
    const leases = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowLeasesForRun(trainingRunRef, 1000),
    })
    const challenges = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listVerificationChallengesForRun(trainingRunRef, 1000),
    })

    return noStoreJsonResponse({
      isoflop: publicScalingSweepProjection({
        challenges,
        leases,
        run: stored,
        windows,
      }),
      run: publicTrainingRunProjection(stored, nowIso),
    })
  })

const routeAttachRealGradientEvidence = <
  Bindings extends TrainingRunWindowRouteEnv,
>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(request, Cs336A1RealGradientEvidenceRequest)
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const run = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.readRun(trainingRunRef),
    })

    if (run === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found.',
      })
    }

    const admitted = yield* Effect.try({
      catch: error =>
        new TrainingAuthorityStoreError({
          kind: 'validation_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        admitCs336A1RealGradientEvidence({ nowIso, request: body, run }),
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.attachRunEvidence(admitted),
    })
    const windows = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowsForRun(trainingRunRef, 100),
    })
    const leases = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowLeasesForRun(trainingRunRef, 1000),
    })
    const challenges = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listVerificationChallengesForRun(trainingRunRef, 1000),
    })
    const summary = publicTrainingRunSummary({
      challenges,
      leases,
      nowIso,
      run: stored,
      windows,
    })

    return noStoreJsonResponse({
      realGradient: summary.realGradient,
      run: publicTrainingRunProjection(stored, nowIso),
    })
  })

const routeAttachDataRefineryEvidence = <
  Bindings extends TrainingRunWindowRouteEnv,
>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(request, Cs336A4DataRefineryEvidenceRequest)
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const run = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.readRun(trainingRunRef),
    })

    if (run === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found.',
      })
    }

    const admitted = yield* Effect.try({
      catch: error =>
        new TrainingAuthorityStoreError({
          kind: 'validation_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        admitCs336A4DataRefineryEvidence({ nowIso, request: body, run }),
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.attachRunEvidence(admitted),
    })
    const windows = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowsForRun(trainingRunRef, 100),
    })
    const leases = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowLeasesForRun(trainingRunRef, 1000),
    })
    const challenges = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listVerificationChallengesForRun(trainingRunRef, 1000),
    })

    return noStoreJsonResponse({
      refinery: publicDataRefineryProjection({
        challenges,
        leases,
        run: stored,
        windows,
      }),
      run: publicTrainingRunProjection(stored, nowIso),
    })
  })

const routeAttachAlignmentEvalEvidence = <
  Bindings extends TrainingRunWindowRouteEnv,
>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(request, Cs336A5AlignmentEvidenceRequest)
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const run = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.readRun(trainingRunRef),
    })

    if (run === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found.',
      })
    }

    const admitted = yield* Effect.try({
      catch: error =>
        new TrainingAuthorityStoreError({
          kind: 'validation_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        admitCs336A5AlignmentEvidence({ nowIso, request: body, run }),
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.attachRunEvidence(admitted),
    })
    const windows = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowsForRun(trainingRunRef, 100),
    })
    const leases = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowLeasesForRun(trainingRunRef, 1000),
    })
    const challenges = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listVerificationChallengesForRun(trainingRunRef, 1000),
    })

    return noStoreJsonResponse({
      evals: publicCs336A5EvalProjection({
        challenges,
        leases,
        run: stored,
        windows,
      }),
      run: publicTrainingRunProjection(stored, nowIso),
    })
  })

const routeReadWindow = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
  windowRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const record = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => dependencies.makeStore(env).readWindow(windowRef),
    })

    if (record === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training window not found.',
      })
    }

    return noStoreJsonResponse({
      window: publicTrainingWindowProjection(record, nowIso),
    })
  })

export const makeTrainingRunWindowRoutes = <
  Bindings extends TrainingRunWindowRouteEnv,
>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
) => ({
  routeTrainingRunWindowRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/training/runs') {
      if (request.method === 'GET') {
        return routeListRuns(dependencies, env).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        )
      }

      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['GET', 'POST']))
      }

      return routePlanRun(dependencies, request, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname === '/api/training/leaderboards/a1') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeA1Leaderboard(dependencies, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname === '/api/training/leaderboards') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeTrainingLeaderboards(dependencies, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    const leaderboardLaneMatch =
      /^\/api\/training\/leaderboards\/([^/]+)$/.exec(url.pathname)

    if (leaderboardLaneMatch !== null) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      const lane = decodeURIComponent(leaderboardLaneMatch[1]!)

      if (!TrainingLeaderboardLanes.some(knownLane => knownLane === lane)) {
        return Effect.succeed(
          noStoreJsonResponse(
            {
              error: 'training_leaderboard_lane_not_found',
              reason: 'Training leaderboard lane not found.',
            },
            { status: 404 },
          ),
        )
      }

      return routeTrainingLeaderboards(dependencies, env, lane).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname === '/api/training/device-capabilities/a2') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeA2DeviceCapabilities(dependencies, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname === '/api/training/evals/a5') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeA5EvalSuites(dependencies, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname === '/api/training/isoflop/a3') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeA3IsoFlop(dependencies, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname === '/api/training/refinery/a4') {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeA4DataRefinery(dependencies, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname === '/api/training/windows/plan') {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routePlanWindow(dependencies, request, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname === '/api/training/leases/claim') {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeClaimLease(dependencies, request, env).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    const bootstrapGrantMatch =
      /^\/api\/training\/runs\/([^/]+)\/bootstrap-grant$/.exec(url.pathname)

    if (bootstrapGrantMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeBootstrapGrant(
        dependencies,
        request,
        env,
        decodeURIComponent(bootstrapGrantMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const runEvidenceMatch =
      /^\/api\/training\/runs\/([^/]+)\/device-benchmark-evidence$/.exec(
        url.pathname,
      )

    if (runEvidenceMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeAttachDeviceBenchmarkEvidence(
        dependencies,
        request,
        env,
        decodeURIComponent(runEvidenceMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const sweepEvidenceMatch =
      /^\/api\/training\/runs\/([^/]+)\/scaling-sweep-evidence$/.exec(
        url.pathname,
      )

    if (sweepEvidenceMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeAttachScalingSweepEvidence(
        dependencies,
        request,
        env,
        decodeURIComponent(sweepEvidenceMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const realGradientEvidenceMatch =
      /^\/api\/training\/runs\/([^/]+)\/real-gradient-evidence$/.exec(
        url.pathname,
      )

    if (realGradientEvidenceMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeAttachRealGradientEvidence(
        dependencies,
        request,
        env,
        decodeURIComponent(realGradientEvidenceMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const refineryEvidenceMatch =
      /^\/api\/training\/runs\/([^/]+)\/data-refinery-evidence$/.exec(
        url.pathname,
      )

    if (refineryEvidenceMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeAttachDataRefineryEvidence(
        dependencies,
        request,
        env,
        decodeURIComponent(refineryEvidenceMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const alignmentEvidenceMatch =
      /^\/api\/training\/runs\/([^/]+)\/alignment-eval-evidence$/.exec(
        url.pathname,
      )

    if (alignmentEvidenceMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeAttachAlignmentEvalEvidence(
        dependencies,
        request,
        env,
        decodeURIComponent(alignmentEvidenceMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const runTransitionMatch =
      /^\/api\/training\/runs\/([^/]+)\/(activate|seal|reconcile)$/.exec(
        url.pathname,
      )

    if (runTransitionMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      const action = runTransitionMatch[2]!
      const nextState: TrainingRunState =
        action === 'activate'
          ? 'active'
          : action === 'seal'
            ? 'sealed'
            : 'reconciled'

      return routeTransitionRun(
        dependencies,
        request,
        env,
        decodeURIComponent(runTransitionMatch[1]!),
        nextState,
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const runAdmitMatch =
      /^\/api\/training\/runs\/([^/]+)\/admit$/.exec(url.pathname)

    if (runAdmitMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      return routeAdmitRunContributor(
        dependencies,
        request,
        env,
        decodeURIComponent(runAdmitMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const runReadMatch = /^\/api\/training\/runs\/([^/]+)$/.exec(url.pathname)

    if (runReadMatch !== null) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeReadRun(
        dependencies,
        env,
        decodeURIComponent(runReadMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const windowTransitionMatch =
      /^\/api\/training\/windows\/([^/]+)\/(activate|seal|reconcile)$/.exec(
        url.pathname,
      )

    if (windowTransitionMatch !== null) {
      if (request.method !== 'POST') {
        return Effect.succeed(methodNotAllowed(['POST']))
      }

      const action = windowTransitionMatch[2]!
      const nextState =
        action === 'activate'
          ? 'active'
          : action === 'seal'
            ? 'sealed'
            : 'reconciled'

      return routeTransitionWindow(
        dependencies,
        request,
        env,
        decodeURIComponent(windowTransitionMatch[1]!),
        `window_${action}`,
        nextState,
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    const windowReadMatch = /^\/api\/training\/windows\/([^/]+)$/.exec(
      url.pathname,
    )

    if (windowReadMatch !== null) {
      if (request.method !== 'GET') {
        return Effect.succeed(methodNotAllowed(['GET']))
      }

      return routeReadWindow(
        dependencies,
        env,
        decodeURIComponent(windowReadMatch[1]!),
      ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))
    }

    return undefined
  },
})
