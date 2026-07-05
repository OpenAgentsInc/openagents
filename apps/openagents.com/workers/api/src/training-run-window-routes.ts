import { Effect, Match as M, Schema as S } from 'effect'

import { publicCs336A5EvalProjection } from './cs336-a5-alignment-homework'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import type { NexusTreasuryPayoutLedgerStore } from './nexus-treasury-payout-ledger'
import { liveAtReadStaleness } from './public-projection-staleness'
import {
  type PublicTassadarSettlementRow,
  resolveRunSettlements as resolveRunSettlementRows,
} from './public-tassadar-run-summary-routes'
import { currentIsoTimestamp, randomUuid } from './runtime-primitives'
import { assertSettledFeedPayloadPublicSafe } from './tassadar-settled-feed-sync'
import {
  TassadarExecutorTraceCloseoutEvidenceSchema,
  tassadarExecutorTraceVerificationChallengeRequest,
} from './tassadar-executor-trace-homework'
import {
  TrainingRunAdmissionRequest,
  decideTassadarRunAdmission,
} from './tassadar-run-admission'
import {
  TassadarRunSettlementRequest,
  TassadarRunSettlementUnsafe,
  buildTassadarRunSettlement,
} from './tassadar-run-settlement'
import {
  readTassadarRealSettlementGate,
  resolveTassadarSettlementAdapter,
} from './tassadar-run-settlement-gate'
import {
  Cs336A5AlignmentEvidenceRequest,
  admitCs336A5AlignmentEvidence,
} from './training-alignment-evals'
import {
  Cs336A4DataRefineryEvidenceRequest,
  Cs336A4RequiredVerifiedStageCount,
  admitCs336A4DataRefineryEvidence,
  aggregateDataRefineryEvalDeltaPaymentGate,
  corpusProvenanceReceiptBlockerRefs,
  corpusProvenanceReceiptStatus,
  publicDataRefineryProjection,
} from './training-data-refinery'
import {
  Cs336A2DeviceBenchmarkEvidenceRequest,
  admitCs336A2DeviceBenchmarkEvidence,
  buildDeviceCapabilitySameClassReplicationSignals,
  buildDeviceCapabilityThermalThrottleSignals,
  publicDeviceCapabilityProjection,
  sameClassReplicationBlockerRefs,
  sameClassReplicationStatus,
  thermalThrottleBlockerRefs,
  thermalThrottleDetectionStatus,
  thermalThrottleFunnelReasonCodes,
  thermalThrottleReceiptRefs,
} from './training-device-capability'
import {
  TrainingLeaderboardLanes,
  buildTrainingLeaderboardsProjection,
} from './training-leaderboards'
import {
  Cs336A1RealGradientEvidenceRequest,
  admitCs336A1RealGradientEvidence,
} from './training-real-gradient-evidence'
import {
  type TrainingAuthorityStore,
  TrainingAuthorityStoreError,
  TrainingRunPlanRequest,
  type TrainingRunProjection,
  type TrainingRunPublicSummary,
  type TrainingRunState,
  TrainingRunTransitionRequest,
  TrainingWindowLeaseClaimRequest,
  TrainingWindowPlanRequest,
  type TrainingWindowState,
  TrainingWindowTransitionRequest,
  appendTrainingRunReceiptRefs,
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
  evaluateUntrustedCurtailmentDrill,
  malformedCurtailmentDrillGate,
} from './training-curtailment-drill'
import {
  evaluateUntrustedStandbyDispatch,
  malformedStandbyDispatchGate,
} from './training-standby-dispatch'
import {
  type TrainingVerificationChallengeCreateRequest,
  type TrainingVerificationChallengeRecord,
  publicTrainingVerificationChallengeProjection,
} from './training-verification'
import {
  TrainingWindowBootstrapGrantRequest,
  decideTrainingWindowBootstrapGrant,
} from './training-window-bootstrap'
import {
  TreasuryPaymentAuthorityError,
  type TreasuryPaymentAuthorityShape,
  type TreasuryPaymentAuthorityWalletReadiness,
} from './treasury-payment-authority'

type HttpResponse = globalThis.Response

type TrainingRunWindowRouteDependencies<Bindings> = Readonly<{
  createVerificationChallenge?: (
    env: Bindings,
    request: TrainingVerificationChallengeCreateRequest,
  ) => Promise<TrainingVerificationChallengeRecord>
  makeId?: () => string
  // The treasury payout ledger store both reads provider-confirmed settlement
  // receipts (for settledPayoutSats projections) and writes the run-tied
  // settlement chain (openagents #5009).
  makePayoutLedgerStore?: (env: Bindings) => NexusTreasuryPayoutLedgerStore
  // REAL-settlement wiring (openagents #5232, Gate 2). All optional: when the
  // owner gate is OFF (the default, everywhere), none of these are consulted and
  // the route is byte-for-byte the current simulation. The real branch is only
  // reachable when `resolveTassadarSettlementAdapter` authorizes the real
  // adapter (gate enabled + allowlisted + under cap), and then it requires the
  // payment authority + wallet readiness + destination resolver to be wired.
  //
  // The payment authority drives the proven Spark treasury rail
  // (`makeSparkTreasuryPayoutAdapter` through `makeTreasuryPaymentAuthority`):
  // deterministic idempotency-keyed dispatch, no-double-pay dedupe, redaction,
  // pause/cap/wallet-readiness gates. Reuse, do not rebuild.
  makeSettlementPaymentAuthority?: (
    env: Bindings,
    context: Readonly<{
      adapterKind: 'spark_treasury'
      ledgerStore: NexusTreasuryPayoutLedgerStore
      privatePayoutDestination: string
      providerRef: string
    }>,
  ) => TreasuryPaymentAuthorityShape
  // Resolve the (private, never-projected) payout destination for the gated
  // recipient. The destination never enters any receipt projection; only the
  // adapter's redacted refs do.
  resolveSettlementPayoutDestination?: (
    env: Bindings,
    contributorRef: string,
  ) => Promise<string | undefined>
  makeStore: (env: Bindings) => TrainingAuthorityStore
  nowIso?: () => string
  // Fresh wallet-readiness evidence for the real payout authority gate. Absent
  // or stale readiness fails the real dispatch closed (no payout).
  readSettlementWalletReadiness?: (
    env: Bindings,
  ) => Promise<TreasuryPaymentAuthorityWalletReadiness>
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
}>

const TrainingRunExecutorTraceCloseoutRequest = S.Struct({
  closeout: TassadarExecutorTraceCloseoutEvidenceSchema,
  windowRef: S.Trim.check(S.isNonEmpty(), S.isMaxLength(260)),
})

type TrainingRunWindowRouteEnv = Readonly<Record<string, unknown>>

class TrainingRunWindowUnauthorized extends S.TaggedErrorClass<TrainingRunWindowUnauthorized>()(
  'TrainingRunWindowUnauthorized',
  {},
) {}

type TrainingRunWindowRouteError =
  | TrainingAuthorityStoreError
  | TrainingRunWindowUnauthorized

const uniqueRouteRefs = (
  refs: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> =>
  [
    ...new Set(refs.map(ref => ref?.trim() ?? '').filter(ref => ref !== '')),
  ].sort()

const trainingRunAggregateStaleness = liveAtReadStaleness([
  'training_run_record_changed',
  'training_window_record_changed',
  'training_window_lease_record_changed',
  'training_verification_challenge_recorded',
  'nexus_treasury_payout_receipt_recorded',
])

type TrainingRunPublicReadEnvelope = Readonly<{
  generatedAt: string
  run: TrainingRunProjection
  sourceRefs: ReadonlyArray<string>
  // Live-at-read staleness contract from publicTrainingRunProjection
  // (maxStalenessSeconds: 0; shared vocabulary in public-projection-staleness).
  staleness: TrainingRunProjection['staleness']
  summary: TrainingRunPublicSummary
}>

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

const requireMethod = (
  request: Request,
  allowed: ReadonlyArray<string>,
  handler: () => Effect.Effect<HttpResponse>,
): Effect.Effect<HttpResponse> =>
  allowed.some(method => method === request.method)
    ? handler()
    : Effect.succeed(methodNotAllowed([...allowed]))

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
          try: () => store.beginRunSealBarrier(current.trainingRunRef, nowIso),
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

    return noStoreJsonResponse({
      admission,
      trainingRunRef: run.trainingRunRef,
    })
  })

const routeExecutorTraceCloseout = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(
      request,
      TrainingRunExecutorTraceCloseoutRequest,
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

    const window = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.readWindow(body.windowRef),
    })

    if (window === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training window not found.',
      })
    }

    if (window.trainingRunRef !== run.trainingRunRef) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'conflict',
        reason: 'Training window does not belong to this run.',
      })
    }

    const createVerificationChallenge = dependencies.createVerificationChallenge

    if (createVerificationChallenge === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'storage_error',
        reason: 'Verification challenge creation is not configured.',
      })
    }

    // The builder enforces the distinct-validator-device rule (exact_trace_replay
    // requires the validator device to differ from the worker Pylon); a violation
    // surfaces as a 400 validation error.
    const challengeRequest = yield* Effect.try({
      catch: error =>
        new TrainingAuthorityStoreError({
          kind: 'validation_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () =>
        tassadarExecutorTraceVerificationChallengeRequest({
          closeout: body.closeout,
          trainingRunRef: run.trainingRunRef,
          windowRef: window.windowRef,
        }),
    })
    const stored = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => createVerificationChallenge(env, challengeRequest),
    })

    return noStoreJsonResponse({
      challenge: publicTrainingVerificationChallengeProjection(stored, nowIso),
      trainingRunRef: run.trainingRunRef,
    })
  })

const settlementErrorFromUnsafe = (
  error: TassadarRunSettlementUnsafe,
): TrainingAuthorityStoreError =>
  new TrainingAuthorityStoreError({
    kind: error.kind,
    reason: error.reason,
  })

// Map a payment-authority payout failure onto the route's typed error. Malformed
// amount/target are caller-validation (400); every other failure (paused,
// stale wallet readiness, missing approval, adapter unavailable, replayed key)
// is an operational conflict (409). Either way the route surfaces a typed error
// and never writes a real-settled receipt — no "paid" claim without a payout.
const settlementErrorFromAuthority = (
  error: TreasuryPaymentAuthorityError,
): TrainingAuthorityStoreError =>
  new TrainingAuthorityStoreError({
    kind:
      error.reason === 'malformed_payout_amount' ||
      error.reason === 'malformed_payout_target'
        ? 'validation_error'
        : 'conflict',
    reason: `real_settlement_payout_blocked:${error.reason}`,
  })

/**
 * Drive a REAL Bitcoin run-settlement payout (openagents #5232, Gate 2).
 *
 * Only reached when the owner gate authorizes the real `spark_treasury` adapter
 * (gate enabled + recipient/run allowlisted + amount under the gate cap). It is
 * RECEIPT-FIRST and IDEMPOTENT:
 *
 * 1. If a settlement receipt already exists for this run/window+recipient
 *    (deterministic ref derived from the request `idempotencyRef`), return
 *    immediately — a retry never re-dispatches.
 * 2. Otherwise drive `TreasuryPaymentAuthority.createPayoutIntent` ->
 *    `dispatchPayout` (Spark adapter) -> `reconcilePayout`, keyed by the
 *    builder's deterministic idempotency-key hashes. The authority dedupes on
 *    the intent key (rejects replay) and the attempt key (returns the existing
 *    attempt without re-dispatching), so at most ONE Spark dispatch occurs per
 *    run/window+recipient.
 * 3. Persist the `settlement_recorded` receipt ONLY after a confirmed dispatch
 *    + matched reconciliation. The persisted records carry the builder's
 *    `moneyMovement:'real_bitcoin'` / matched / settled projection, so
 *    `nexus-pylon-visibility.ts` derives `realBitcoinMoved:true`.
 *
 * On any payout failure no real-settled receipt is written and a typed error is
 * surfaced.
 */
// Injected resolvers for the proven receipt-first Spark dispatch. Extracted so
// both the admin settlement route and the hands-off auto-stream path
// (openagents #5309/#5310) drive the SAME dispatch logic instead of two copies.
export type RealRunSettlementDispatchResolvers<Bindings> = Readonly<{
  env: Bindings
  makeSettlementPaymentAuthority: NonNullable<
    TrainingRunWindowRouteDependencies<Bindings>['makeSettlementPaymentAuthority']
  >
  readSettlementWalletReadiness: NonNullable<
    TrainingRunWindowRouteDependencies<Bindings>['readSettlementWalletReadiness']
  >
  resolveSettlementPayoutDestination: NonNullable<
    TrainingRunWindowRouteDependencies<Bindings>['resolveSettlementPayoutDestination']
  >
}>

/**
 * Receipt-first, idempotent Spark dispatch core (openagents #5232). Reused by
 * the admin settlement route and the auto-stream path. Resolves wallet
 * readiness + private destination, drives intent -> dispatch -> reconcile keyed
 * by the builder's deterministic idempotency hashes (so a retry pays AT MOST
 * ONCE), and persists the `settlement_recorded` receipt ONLY after a confirmed
 * dispatch + matched reconciliation. Any failure surfaces a typed error and
 * writes NO real-settled receipt — no "paid" claim without a confirmed payout.
 */
export const dispatchRealRunSettlementCore = <Bindings>(
  resolvers: RealRunSettlementDispatchResolvers<Bindings>,
  input: Readonly<{
    contributorRef: string
    ledger: NexusTreasuryPayoutLedgerStore
    settlement: ReturnType<typeof buildTassadarRunSettlement>
  }>,
): Effect.Effect<void, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const { contributorRef, ledger, settlement } = input
    const {
      env,
      makeSettlementPaymentAuthority: makeAuthority,
      readSettlementWalletReadiness: readWalletReadiness,
      resolveSettlementPayoutDestination: resolveDestination,
    } = resolvers

    // Idempotent short-circuit: if the deterministic receipt already exists,
    // this run/window+recipient already settled. Do not dispatch again.
    const existingReceipt = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () =>
        ledger.readPaymentAuthorityReceiptByRef(
          settlement.settlementReceiptRef,
        ),
    })

    if (existingReceipt !== undefined) {
      return
    }

    const walletReadiness = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => readWalletReadiness(env),
    })

    if (walletReadiness !== 'ready') {
      return yield* new TrainingAuthorityStoreError({
        kind: 'conflict',
        reason: `real_settlement_payout_blocked:stale_or_absent_wallet_readiness:${walletReadiness}`,
      })
    }

    const privatePayoutDestination = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => resolveDestination(env, contributorRef),
    })

    if (
      privatePayoutDestination === undefined ||
      privatePayoutDestination.trim() === ''
    ) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'conflict',
        reason: 'real_settlement_payout_blocked:missing_payout_destination',
      })
    }

    const authority = makeAuthority(env, {
      adapterKind: 'spark_treasury',
      ledgerStore: ledger,
      privatePayoutDestination,
      providerRef: settlement.reconciliationEvent.providerRef,
    })

    // The payout-target approval is a foreign key for the intent; write it
    // first (matches the simulation path and the proven accepted-work bridge).
    yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => ledger.createPayoutTargetApproval(settlement.targetApproval),
    })

    // createPayoutIntent rejects a replayed idempotency key. On retry, the
    // intent already exists; reuse it rather than failing the whole route.
    const existingIntent = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () =>
        ledger.readPayoutIntentByIdempotencyKeyHash(
          settlement.intent.idempotencyKeyHash,
        ),
    })

    if (existingIntent === undefined) {
      yield* authority
        .createPayoutIntent({
          intent: settlement.intent,
          walletReadiness,
        })
        .pipe(Effect.mapError(settlementErrorFromAuthority))
    }

    // dispatchPayout reads the existing attempt by idempotency key first and
    // returns it WITHOUT re-dispatching, so a retry dispatches at most once.
    const dispatch = yield* authority
      .dispatchPayout({
        attempt: settlement.attempt,
        payoutIntentRef: settlement.intent.payoutIntentRef,
      })
      .pipe(Effect.mapError(settlementErrorFromAuthority))

    const reconciliation = yield* authority
      .reconcilePayout({ event: settlement.reconciliationEvent })
      .pipe(Effect.mapError(settlementErrorFromAuthority))

    // Receipt-first guard: only a confirmed dispatch + matched reconciliation
    // may back a real-settled receipt. Anything else fails closed.
    if (
      dispatch.attempt.status !== 'dispatched' ||
      reconciliation.event.status !== 'matched'
    ) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'conflict',
        reason: `real_settlement_payout_unconfirmed:${dispatch.attempt.status}:${reconciliation.event.status}`,
      })
    }

    // Now — and only now — persist the settlement_recorded receipt. Its
    // projection already carries moneyMovement:'real_bitcoin' / settled, so the
    // public derivation yields realBitcoinMoved:true.
    yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () =>
        ledger.createPaymentAuthorityReceipt(settlement.settlementReceipt),
    })
  })

const dispatchRealRunSettlement = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
  input: Readonly<{
    contributorRef: string
    ledger: NexusTreasuryPayoutLedgerStore
    nowIso: string
    settlement: ReturnType<typeof buildTassadarRunSettlement>
  }>,
): Effect.Effect<void, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const makeAuthority = dependencies.makeSettlementPaymentAuthority
    const readWalletReadiness = dependencies.readSettlementWalletReadiness
    const resolveDestination = dependencies.resolveSettlementPayoutDestination

    if (
      makeAuthority === undefined ||
      readWalletReadiness === undefined ||
      resolveDestination === undefined
    ) {
      // The gate authorized a real payout but the live dispatch path is not
      // wired in this deployment. Fail closed (no money, no receipt).
      return yield* new TrainingAuthorityStoreError({
        kind: 'storage_error',
        reason:
          'Real settlement is gate-authorized but the payout authority is not configured.',
      })
    }

    yield* dispatchRealRunSettlementCore(
      {
        env,
        makeSettlementPaymentAuthority: makeAuthority,
        readSettlementWalletReadiness: readWalletReadiness,
        resolveSettlementPayoutDestination: resolveDestination,
      },
      {
        contributorRef: input.contributorRef,
        ledger: input.ledger,
        settlement: input.settlement,
      },
    )
  })

/**
 * Settle one accepted (Verified) executor-trace work item for a run
 * (openagents #5009, JUNE15_LAUNCH_PLAN §4.D — the earn-Bitcoin leg). Records
 * the operator-approved treasury payout chain (intent -> attempt ->
 * reconciliation -> settlement_recorded receipt) under the run spend cap and
 * links the public settlement receipt back onto the run, so the run summary
 * `providerConfirmedSettledPayoutSats` and the A1 leaderboard `settledPayoutSats`
 * reflect real confirmed settlement. Admin-only.
 */
const routeRunSettlementReceipt = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* decodeBody(request, TassadarRunSettlementRequest)
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const makePayoutLedgerStore = dependencies.makePayoutLedgerStore

    if (makePayoutLedgerStore === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'storage_error',
        reason: 'Treasury payout ledger is not configured.',
      })
    }

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

    const challenges = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listVerificationChallengesForRun(trainingRunRef, 1000),
    })
    const challenge = challenges.find(
      candidate => candidate.challengeRef === body.challengeRef,
    )

    if (challenge === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Verification challenge not found for this run.',
      })
    }

    const leases = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowLeasesForRun(trainingRunRef, 1000),
    })
    const lease = leases.find(candidate => candidate.leaseRef === body.leaseRef)

    if (lease === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Window lease not found for this run.',
      })
    }

    // Resolve the settlement adapter through the owner gate (openagents #5232).
    // DEFAULT OFF: with no `OPENAGENTS_REAL_SETTLEMENT_GATE` set (the state
    // everywhere today) this always resolves to `simulation`, even if an admin
    // passes `spark_treasury`. The resolved kind — not the raw request kind —
    // is what drives the builder, so the simulation path is byte-for-byte
    // unchanged. The real branch is only reachable when the gate is enabled AND
    // the recipient/run are allowlisted AND the amount is under the gate cap.
    const contributorRef = lease.pylonRef.trim()
    const settlementDecision = resolveTassadarSettlementAdapter({
      amountSats: body.amountSats,
      contributorRef,
      gate: readTassadarRealSettlementGate(env),
      requestedAdapterKind: body.adapterKind ?? 'simulation',
      trainingRunRef,
    })

    const settlement = yield* Effect.try({
      catch: error =>
        error instanceof TassadarRunSettlementUnsafe
          ? settlementErrorFromUnsafe(error)
          : new TrainingAuthorityStoreError({
              kind: 'validation_error',
              reason: error instanceof Error ? error.message : String(error),
            }),
      try: () =>
        buildTassadarRunSettlement({
          challenge,
          lease,
          nowIso,
          // The resolved (gated) adapter kind overrides the request. Anything
          // not authorized by the gate builds the simulation chain.
          request: { ...body, adapterKind: settlementDecision.adapterKind },
          run,
        }),
    })
    const ledger = makePayoutLedgerStore(env)

    if (settlementDecision.realAuthorized) {
      // REAL Bitcoin branch (openagents #5232). Receipt-first: dispatch the
      // payout through the proven Spark treasury rail before the
      // settlement_recorded receipt is ever written, keyed by the builder's
      // deterministic idempotency-key hashes so a retry pays AT MOST ONCE. On
      // any payout failure we write NO real-settled receipt and surface a typed
      // error (no "paid" claim without a confirmed real payout).
      yield* dispatchRealRunSettlement(dependencies, env, {
        contributorRef,
        ledger,
        nowIso,
        settlement,
      })
    } else {
      yield* Effect.tryPromise({
        catch: trainingAuthorityStoreErrorFromUnknown,
        try: async () => {
          // The payout intent foreign-keys the payout-target approval, so the
          // approval row must be written first (see tassadar-run-settlement.ts).
          await ledger.createPayoutTargetApproval(settlement.targetApproval)
          await ledger.createPayoutIntent(settlement.intent)
          await ledger.createPayoutAttempt(settlement.attempt)
          await ledger.createReconciliationEvent(settlement.reconciliationEvent)
          await ledger.createPaymentAuthorityReceipt(
            settlement.settlementReceipt,
          )
        },
      })
    }

    const linked = appendTrainingRunReceiptRefs({
      nowIso,
      receiptRefs: [settlement.settlementReceiptRef],
      run,
    })
    const storedRun = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.transitionRun(linked.run),
    })
    const windows = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.listWindowsForRun(trainingRunRef, 100),
    })
    const resolved = yield* resolveRunSettlements(dependencies, env, [
      ...storedRun.receiptRefs,
      ...windows.flatMap(window => window.receiptRefs),
      ...leases.flatMap(candidate => candidate.receiptRefs),
      ...challenges.flatMap(candidate => candidate.verdictRefs),
    ])

    return noStoreJsonResponse({
      run: publicTrainingRunProjection(storedRun, nowIso),
      settlement: {
        amountSats: settlement.amountSats,
        contributorRef: settlement.contributorRef,
        settlementReceiptRef: settlement.settlementReceiptRef,
        verificationChallengeRef: challenge.challengeRef,
      },
      summary: publicTrainingRunSummary({
        challenges,
        leases,
        nowIso,
        run: storedRun,
        settledSatsByReceiptRef: resolved.settledSatsByReceiptRef,
        settlementReceiptRefsByContributor:
          resolved.settlementReceiptRefsByContributor,
        windows,
      }),
    })
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

const routeStandbyDispatchPreflight = <
  Bindings extends TrainingRunWindowRouteEnv,
>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* Effect.tryPromise({
      catch: error =>
        new TrainingAuthorityStoreError({
          kind: 'validation_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => readJsonObject(request),
    })
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

    const requestedRunRef = body.runRef
    const standbyDispatch =
      requestedRunRef !== undefined && requestedRunRef !== trainingRunRef
        ? malformedStandbyDispatchGate()
        : evaluateUntrustedStandbyDispatch({
            ...body,
            runRef: trainingRunRef,
          })

    return noStoreJsonResponse({
      run: publicTrainingRunProjection(run, nowIso),
      standbyDispatch,
    })
  })

const routeCurtailmentDrillPreflight = <
  Bindings extends TrainingRunWindowRouteEnv,
>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)
    const body = yield* Effect.tryPromise({
      catch: error =>
        new TrainingAuthorityStoreError({
          kind: 'validation_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: () => readJsonObject(request),
    })
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

    const requestedRunRef = body.runRef
    const curtailmentDrill =
      requestedRunRef !== undefined && requestedRunRef !== trainingRunRef
        ? malformedCurtailmentDrillGate()
        : evaluateUntrustedCurtailmentDrill({
            ...body,
            runRef: trainingRunRef,
          })

    return noStoreJsonResponse({
      curtailmentDrill,
      run: publicTrainingRunProjection(run, nowIso),
    })
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

const readRunPublicEnvelope = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
  trainingRunRef: string,
  publicRouteRef?: string,
): Effect.Effect<TrainingRunPublicReadEnvelope, TrainingRunWindowRouteError> =>
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
    const settlement = yield* resolveRunSettlements(dependencies, env, [
      ...record.receiptRefs,
      ...windows.flatMap(window => window.receiptRefs),
      ...leases.flatMap(lease => lease.receiptRefs),
      ...challenges.flatMap(challenge => challenge.verdictRefs),
    ])
    const baseSummary = publicTrainingRunSummary({
      challenges,
      leases,
      nowIso,
      run: record,
      settledSatsByReceiptRef: settlement.settledSatsByReceiptRef,
      settlementReceiptRefsByContributor:
        settlement.settlementReceiptRefsByContributor,
      windows,
    })
    const summary = {
      ...baseSummary,
      sourceRefs: uniqueRouteRefs([...baseSummary.sourceRefs, publicRouteRef]),
    }

    return {
      generatedAt: nowIso,
      run: summary.run,
      sourceRefs: summary.sourceRefs,
      staleness: summary.run.staleness,
      summary,
    }
  })

const routeReadRun = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
  trainingRunRef: string,
  publicRouteRef?: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  readRunPublicEnvelope(dependencies, env, trainingRunRef, publicRouteRef).pipe(
    Effect.map(envelope => noStoreJsonResponse(envelope)),
  )

export const TrainingRunSettlementsSchemaVersion =
  'openagents.training_run_settlements.v1'

// Public-safe, enumerable settled feed keyed by run (openagents #5316). Reuses
// the SAME provider-confirmed settled-receipt resolution path that feeds
// metrics.providerConfirmedSettledPayoutSats (the exported resolveRunSettlements
// from public-tassadar-run-summary-routes), so any contributor can enumerate and
// dereference their own run-linked payout without trusting a forum post. Every
// row is run through the public-safe guard before it leaves the Worker; a row
// that ever scans unsafe surfaces as a generic storage error rather than leaking
// raw payment material.
const routeReadRunSettlements = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  Effect.gen(function* () {
    const nowIso = routeNowIso(dependencies)
    const store = dependencies.makeStore(env)
    const record = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => store.readRun(trainingRunRef),
    })

    if (record === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'Training run not found.',
      })
    }

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

    const receiptRefs = [
      ...record.receiptRefs,
      ...windows.flatMap(window => window.receiptRefs),
      ...leases.flatMap(lease => lease.receiptRefs),
      ...challenges.flatMap(challenge => challenge.verdictRefs),
    ]
    const appUrl = new URL(request.url).origin
    const payoutLedgerStore = dependencies.makePayoutLedgerStore?.(env)
    const resolution = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () =>
        resolveRunSettlementRows(payoutLedgerStore, receiptRefs, appUrl),
    })
    const settlementRows: ReadonlyArray<PublicTassadarSettlementRow> =
      resolution.settlementRows

    // Server-side public-safe guarantee: never let a settlement row leave the
    // Worker if it scans for raw payment material. Surfaces as a generic typed
    // storage error (no raw material in the reason).
    yield* Effect.try({
      catch: () =>
        new TrainingAuthorityStoreError({
          kind: 'storage_error',
          reason: 'Settlement row failed the public-safe guard.',
        }),
      try: () => {
        for (const row of settlementRows) {
          assertSettledFeedPayloadPublicSafe(
            'Training run settlements row',
            row,
          )
        }
      },
    })

    return noStoreJsonResponse({
      generatedAt: nowIso,
      runRef: record.trainingRunRef,
      schemaVersion: TrainingRunSettlementsSchemaVersion,
      staleness: liveAtReadStaleness([
        'training_run_state_transition_recorded',
        'training_window_state_transition_recorded',
        'training_run_evidence_attached',
      ]),
      settlementRows,
      sourceRefs: [
        `route:/api/public/training/runs/${record.trainingRunRef}/settlements`,
        `route:/api/training/runs/${record.trainingRunRef}/settlements`,
        'route:/api/training/runs',
      ],
    })
  })

const routeReadPublicRun = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
  trainingRunRef: string,
): Effect.Effect<HttpResponse, TrainingRunWindowRouteError> =>
  routeReadRun(
    dependencies,
    env,
    trainingRunRef,
    `route:/api/public/training/runs/${trainingRunRef}`,
  )

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
        const settlement = yield* resolveRunSettlements(dependencies, env, [
          ...run.receiptRefs,
          ...windows.flatMap(window => window.receiptRefs),
          ...leases.flatMap(lease => lease.receiptRefs),
          ...challenges.flatMap(challenge => challenge.verdictRefs),
        ])

        return publicTrainingRunSummary({
          challenges,
          leases,
          nowIso,
          run,
          settledSatsByReceiptRef: settlement.settledSatsByReceiptRef,
          settlementReceiptRefsByContributor:
            settlement.settlementReceiptRefsByContributor,
          windows,
        })
      }),
    )

    return noStoreJsonResponse({
      generatedAt: nowIso,
      runs: runs.map(run => publicTrainingRunProjection(run, nowIso)),
      staleness: trainingRunAggregateStaleness,
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
        const settlement = yield* resolveRunSettlements(dependencies, env, [
          ...run.receiptRefs,
          ...windows.flatMap(window => window.receiptRefs),
          ...leases.flatMap(lease => lease.receiptRefs),
          ...challenges.flatMap(challenge => challenge.verdictRefs),
        ])

        return publicTrainingRunSummary({
          challenges,
          leases,
          nowIso,
          run,
          settledSatsByReceiptRef: settlement.settledSatsByReceiptRef,
          settlementReceiptRefsByContributor:
            settlement.settlementReceiptRefsByContributor,
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
      generatedAt: nowIso,
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
      staleness: trainingRunAggregateStaleness,
    })
  })

/**
 * Resolve provider-confirmed settlement for a set of run-linked receipt refs
 * (openagents #5009). Returns settled sats keyed by receipt ref (for the run
 * summary metric and leaderboard sums) plus the settlement receipt refs grouped
 * by contributor (so per-contributor leaderboard rows can surface their settled
 * sats).
 *
 * Delegates to the single exported `resolveRunSettlementRows` (alias of the
 * exported `resolveRunSettlements` in public-tassadar-run-summary-routes) so
 * EVERY run/leaderboard endpoint shares one real-only settlement projection:
 * the real settled-sats total counts ONLY receipts where bitcoin actually moved
 * (`realBitcoinMoved === true`, derived from `movementMode/moneyMovement ===
 * 'real_bitcoin'` + `receiptKind === 'settlement_recorded'` + matched
 * reconciliation eventStatus or `realBitcoinMoved` in the projection). A
 * settled-STATE simulation receipt does NOT inflate the total (1000 + 5 real =
 * 1005, not 1010). This removes the duplicate, real-blind local resolver that
 * produced the sim-vs-real conflation Orrery dereferenced.
 *
 * The async exported resolver is wrapped in `Effect.tryPromise` so it composes
 * inside the route generators. `settlementRows` from the resolution is unused by
 * these call sites (the public summary + settlements feed consume it directly),
 * so `appUrl` only affects discarded row URLs and uses the production default.
 */
const resolveRunSettlements = <Bindings extends TrainingRunWindowRouteEnv>(
  dependencies: TrainingRunWindowRouteDependencies<Bindings>,
  env: Bindings,
  receiptRefs: ReadonlyArray<string>,
): Effect.Effect<
  Awaited<ReturnType<typeof resolveRunSettlementRows>>,
  TrainingRunWindowRouteError
> =>
  Effect.tryPromise({
    catch: trainingAuthorityStoreErrorFromUnknown,
    try: () =>
      resolveRunSettlementRows(
        dependencies.makePayoutLedgerStore?.(env),
        receiptRefs,
        'https://openagents.com',
      ),
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

        return { challenges, leases, run, windows }
      }),
    )
    const baseBuilderInput = {
      a2Projections: inputs.map(input =>
        publicDeviceCapabilityProjection(input),
      ),
      a3Projections: inputs.map(input => publicScalingSweepProjection(input)),
      a5Projections: inputs.map(input => publicCs336A5EvalProjection(input)),
      runs,
    }
    // A draft (no settlement) only to harvest the a2/a3/a4/a5 lane receipt refs,
    // which carry their own settlement receipts independent of the run link
    // (a1 settlement is contributor-linked below). Resolve over those refs plus
    // every run/window/lease/challenge ref so the run summary and all lanes see
    // the same provider-confirmed settlement (openagents #5009).
    const draftSummaries = inputs.map(input =>
      publicTrainingRunSummary({ ...input, nowIso }),
    )
    const draft = buildTrainingLeaderboardsProjection({
      ...baseBuilderInput,
      summaries: draftSummaries,
    })
    const settlement = yield* resolveRunSettlements(dependencies, env, [
      ...inputs.flatMap(input => [
        ...input.run.receiptRefs,
        ...input.windows.flatMap(window => window.receiptRefs),
        ...input.leases.flatMap(lease => lease.receiptRefs),
        ...input.challenges.flatMap(challenge => challenge.verdictRefs),
      ]),
      ...draft.lanes.flatMap(lane => lane.rows.flatMap(row => row.receiptRefs)),
    ])
    const projection = buildTrainingLeaderboardsProjection({
      ...baseBuilderInput,
      settledSatsByReceiptRef: settlement.settledSatsByReceiptRef,
      settlementReceiptRefsByContributor:
        settlement.settlementReceiptRefsByContributor,
      summaries: inputs.map(input =>
        publicTrainingRunSummary({
          ...input,
          nowIso,
          settledSatsByReceiptRef: settlement.settledSatsByReceiptRef,
          settlementReceiptRefsByContributor:
            settlement.settlementReceiptRefsByContributor,
        }),
      ),
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
      generatedAt: nowIso,
      lanes,
      staleness: trainingRunAggregateStaleness,
    })
  })

const routeA3IsoFlop = <Bindings extends TrainingRunWindowRouteEnv>(
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
      generatedAt: nowIso,
      projections,
      schemaVersion: 'openagents.training.isoflop_dashboard.v1',
      sourceRefs: [
        'route:/api/training/isoflop/a3',
        'route:/api/training/runs',
      ],
      staleness: trainingRunAggregateStaleness,
    })
  })

const routeA4DataRefinery = <Bindings extends TrainingRunWindowRouteEnv>(
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
        shards.filter(shard => shard.verified).map(shard => shard.stage),
      ),
    ].sort()
    const stageBlockerRefs =
      verifiedStages.length >= Cs336A4RequiredVerifiedStageCount
        ? []
        : [
            'blocker.cs336_a4.requires_three_verified_stages',
            'blocker.cs336_a4.operator_funding_required_for_paid_shards',
          ]
    const provenanceBlockerRefs = corpusProvenanceReceiptBlockerRefs(shards)

    return noStoreJsonResponse({
      blockerRefs: uniqueRouteRefs([
        ...stageBlockerRefs,
        ...provenanceBlockerRefs,
      ]),
      corpusProvenanceReceiptBlockerRefs: provenanceBlockerRefs,
      corpusProvenanceReceiptRefs: uniqueRouteRefs(
        shards.flatMap(shard =>
          shard.corpusProvenanceReceiptRef === null
            ? []
            : [shard.corpusProvenanceReceiptRef],
        ),
      ),
      corpusProvenanceReceiptStatus: corpusProvenanceReceiptStatus(shards),
      evalDeltaBonusBlockerRefs: [
        'blocker.cs336_a4.fixed_trainer_eval_loop_required_for_quality_bonus',
        'blocker.cs336_a4.operator_funding_required_for_bonus_settlement',
        'blocker.cs336_a4.psionic_classifier_adapters_partial',
      ],
      evalDeltaPaymentGate:
        aggregateDataRefineryEvalDeltaPaymentGate(projections),
      generatedAt: nowIso,
      observedVerifiedStages: verifiedStages,
      projections,
      requiredVerifiedStageCount: Cs336A4RequiredVerifiedStageCount,
      schemaVersion: 'openagents.training.data_refinery_dashboard.v1',
      shards,
      sourceRefs: [
        'route:/api/training/refinery/a4',
        'route:/api/training/runs',
      ],
      staleness: trainingRunAggregateStaleness,
    })
  })

const routeA2DeviceCapabilities = <Bindings extends TrainingRunWindowRouteEnv>(
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
    const observedSettledDeviceClassCount = new Set(
      classDistributions
        .filter(distribution => distribution.verified)
        .map(distribution => distribution.deviceClassRef),
    ).size
    const dashboardSameClassReplicationSignals =
      buildDeviceCapabilitySameClassReplicationSignals(classDistributions)
    const dashboardSameClassReplicationBlockerRefs =
      sameClassReplicationBlockerRefs(dashboardSameClassReplicationSignals)
    const dashboardThermalThrottleSignals =
      buildDeviceCapabilityThermalThrottleSignals(classDistributions)
    const measurementBlockerRefs =
      classDistributions.length > 0 &&
      verifiedCount === classDistributions.length
        ? []
        : [
            'blocker.cs336_a2.requires_receipted_benchmark_results',
            'blocker.cs336_a2.requires_statistical_cross_check',
            'blocker.cs336_a2.requires_replication_across_same_class_devices',
          ]

    return noStoreJsonResponse({
      blockerRefs: [
        ...new Set([
          ...measurementBlockerRefs,
          ...dashboardSameClassReplicationBlockerRefs,
        ]),
      ].sort(),
      classDistributions,
      generatedAt: nowIso,
      observedDeviceClassCount,
      observedMeasurementCount: classDistributions.length,
      observedSettledDeviceClassCount,
      projections,
      schemaVersion: 'openagents.training.device_capability_dashboard.v1',
      sameClassReplicationBlockerRefs:
        dashboardSameClassReplicationBlockerRefs,
      sameClassReplicationSignals: dashboardSameClassReplicationSignals,
      sameClassReplicationStatus: sameClassReplicationStatus(
        dashboardSameClassReplicationSignals,
      ),
      sourceRefs: [
        'route:/api/training/device-capabilities/a2',
        'route:/api/training/runs',
      ],
      thermalThrottleBlockerRefs: thermalThrottleBlockerRefs(
        dashboardThermalThrottleSignals,
      ),
      thermalThrottleDetectionStatus: thermalThrottleDetectionStatus(
        dashboardThermalThrottleSignals,
      ),
      thermalThrottleFunnelReasonCodes: thermalThrottleFunnelReasonCodes(
        dashboardThermalThrottleSignals,
      ),
      thermalThrottleReceiptRefs: thermalThrottleReceiptRefs(
        dashboardThermalThrottleSignals,
      ),
      thermalThrottleSignals: dashboardThermalThrottleSignals,
      staleness: trainingRunAggregateStaleness,
    })
  })

const routeA5EvalSuites = <Bindings extends TrainingRunWindowRouteEnv>(
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
      generatedAt: nowIso,
      projections,
      schemaVersion: 'openagents.training.a5_eval_dashboard.v1',
      sourceRefs: ['route:/api/training/evals/a5', 'route:/api/training/runs'],
      staleness: trainingRunAggregateStaleness,
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
      try: () => admitCs336A5AlignmentEvidence({ nowIso, request: body, run }),
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
      return requireMethod(request, ['GET'], () =>
        routeA1Leaderboard(dependencies, env).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        ),
      )
    }

    if (url.pathname === '/api/training/leaderboards') {
      return requireMethod(request, ['GET'], () =>
        routeTrainingLeaderboards(dependencies, env).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        ),
      )
    }

    const leaderboardLaneMatch =
      /^\/api\/training\/leaderboards\/([^/]+)$/.exec(url.pathname)

    if (leaderboardLaneMatch !== null) {
      return requireMethod(request, ['GET'], () => {
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
      })
    }

    // Public alias for the per-run settlements feed (#5403 gap 2). The data is
    // already public — it lives in the run-summary `settlementRows` under
    // `/api/public/` and the non-`/public/` settlements feed serves the same
    // resolution. A skeptic who curls the public-LOOKING path (with `/public/`)
    // got a 404; serve the identical public-safe handler here so both paths
    // resolve. Matched BEFORE the public run read so the trailing
    // `/settlements` segment is not swallowed by the single-segment run match.
    const publicRunSettlementsListMatch =
      /^\/api\/public\/training\/runs\/([^/]+)\/settlements$/.exec(url.pathname)

    if (publicRunSettlementsListMatch !== null) {
      return requireMethod(request, ['GET'], () =>
        routeReadRunSettlements(
          dependencies,
          request,
          env,
          decodeURIComponent(publicRunSettlementsListMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const publicRunReadMatch = /^\/api\/public\/training\/runs\/([^/]+)$/.exec(
      url.pathname,
    )

    if (publicRunReadMatch !== null) {
      return requireMethod(request, ['GET'], () =>
        routeReadPublicRun(
          dependencies,
          env,
          decodeURIComponent(publicRunReadMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    if (url.pathname === '/api/training/device-capabilities/a2') {
      return requireMethod(request, ['GET'], () =>
        routeA2DeviceCapabilities(dependencies, env).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        ),
      )
    }

    if (url.pathname === '/api/training/evals/a5') {
      return requireMethod(request, ['GET'], () =>
        routeA5EvalSuites(dependencies, env).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        ),
      )
    }

    if (url.pathname === '/api/training/isoflop/a3') {
      return requireMethod(request, ['GET'], () =>
        routeA3IsoFlop(dependencies, env).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        ),
      )
    }

    if (url.pathname === '/api/training/refinery/a4') {
      return requireMethod(request, ['GET'], () =>
        routeA4DataRefinery(dependencies, env).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        ),
      )
    }

    if (url.pathname === '/api/training/windows/plan') {
      return requireMethod(request, ['POST'], () =>
        routePlanWindow(dependencies, request, env).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        ),
      )
    }

    if (url.pathname === '/api/training/leases/claim') {
      return requireMethod(request, ['POST'], () =>
        routeClaimLease(dependencies, request, env).pipe(
          Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
        ),
      )
    }

    const bootstrapGrantMatch =
      /^\/api\/training\/runs\/([^/]+)\/bootstrap-grant$/.exec(url.pathname)

    if (bootstrapGrantMatch !== null) {
      return requireMethod(request, ['POST'], () =>
        routeBootstrapGrant(
          dependencies,
          request,
          env,
          decodeURIComponent(bootstrapGrantMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const standbyDispatchPreflightMatch =
      /^\/api\/training\/runs\/([^/]+)\/standby-dispatch-preflight$/.exec(
        url.pathname,
      )

    if (standbyDispatchPreflightMatch !== null) {
      return requireMethod(request, ['POST'], () =>
        routeStandbyDispatchPreflight(
          dependencies,
          request,
          env,
          decodeURIComponent(standbyDispatchPreflightMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const curtailmentDrillPreflightMatch =
      /^\/api\/training\/runs\/([^/]+)\/curtailment-drill-preflight$/.exec(
        url.pathname,
      )

    if (curtailmentDrillPreflightMatch !== null) {
      return requireMethod(request, ['POST'], () =>
        routeCurtailmentDrillPreflight(
          dependencies,
          request,
          env,
          decodeURIComponent(curtailmentDrillPreflightMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const runEvidenceMatch =
      /^\/api\/training\/runs\/([^/]+)\/device-benchmark-evidence$/.exec(
        url.pathname,
      )

    if (runEvidenceMatch !== null) {
      return requireMethod(request, ['POST'], () =>
        routeAttachDeviceBenchmarkEvidence(
          dependencies,
          request,
          env,
          decodeURIComponent(runEvidenceMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const sweepEvidenceMatch =
      /^\/api\/training\/runs\/([^/]+)\/scaling-sweep-evidence$/.exec(
        url.pathname,
      )

    if (sweepEvidenceMatch !== null) {
      return requireMethod(request, ['POST'], () =>
        routeAttachScalingSweepEvidence(
          dependencies,
          request,
          env,
          decodeURIComponent(sweepEvidenceMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const realGradientEvidenceMatch =
      /^\/api\/training\/runs\/([^/]+)\/real-gradient-evidence$/.exec(
        url.pathname,
      )

    if (realGradientEvidenceMatch !== null) {
      return requireMethod(request, ['POST'], () =>
        routeAttachRealGradientEvidence(
          dependencies,
          request,
          env,
          decodeURIComponent(realGradientEvidenceMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const refineryEvidenceMatch =
      /^\/api\/training\/runs\/([^/]+)\/data-refinery-evidence$/.exec(
        url.pathname,
      )

    if (refineryEvidenceMatch !== null) {
      return requireMethod(request, ['POST'], () =>
        routeAttachDataRefineryEvidence(
          dependencies,
          request,
          env,
          decodeURIComponent(refineryEvidenceMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const alignmentEvidenceMatch =
      /^\/api\/training\/runs\/([^/]+)\/alignment-eval-evidence$/.exec(
        url.pathname,
      )

    if (alignmentEvidenceMatch !== null) {
      return requireMethod(request, ['POST'], () =>
        routeAttachAlignmentEvalEvidence(
          dependencies,
          request,
          env,
          decodeURIComponent(alignmentEvidenceMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const runTransitionMatch =
      /^\/api\/training\/runs\/([^/]+)\/(activate|seal|reconcile)$/.exec(
        url.pathname,
      )

    if (runTransitionMatch !== null) {
      return requireMethod(request, ['POST'], () => {
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
      })
    }

    const runAdmitMatch = /^\/api\/training\/runs\/([^/]+)\/admit$/.exec(
      url.pathname,
    )

    if (runAdmitMatch !== null) {
      return requireMethod(request, ['POST'], () =>
        routeAdmitRunContributor(
          dependencies,
          request,
          env,
          decodeURIComponent(runAdmitMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const executorTraceCloseoutMatch =
      /^\/api\/training\/runs\/([^/]+)\/executor-trace-closeout$/.exec(
        url.pathname,
      )

    if (executorTraceCloseoutMatch !== null) {
      return requireMethod(request, ['POST'], () =>
        routeExecutorTraceCloseout(
          dependencies,
          request,
          env,
          decodeURIComponent(executorTraceCloseoutMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const runSettlementMatch =
      /^\/api\/training\/runs\/([^/]+)\/settlement-receipt$/.exec(url.pathname)

    if (runSettlementMatch !== null) {
      return requireMethod(request, ['POST'], () =>
        routeRunSettlementReceipt(
          dependencies,
          request,
          env,
          decodeURIComponent(runSettlementMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const runSettlementsListMatch =
      /^\/api\/training\/runs\/([^/]+)\/settlements$/.exec(url.pathname)

    if (runSettlementsListMatch !== null) {
      return requireMethod(request, ['GET'], () =>
        routeReadRunSettlements(
          dependencies,
          request,
          env,
          decodeURIComponent(runSettlementsListMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const runReadMatch = /^\/api\/training\/runs\/([^/]+)$/.exec(url.pathname)

    if (runReadMatch !== null) {
      return requireMethod(request, ['GET'], () =>
        routeReadRun(
          dependencies,
          env,
          decodeURIComponent(runReadMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    const windowTransitionMatch =
      /^\/api\/training\/windows\/([^/]+)\/(activate|seal|reconcile)$/.exec(
        url.pathname,
      )

    if (windowTransitionMatch !== null) {
      return requireMethod(request, ['POST'], () => {
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
      })
    }

    const windowReadMatch = /^\/api\/training\/windows\/([^/]+)$/.exec(
      url.pathname,
    )

    if (windowReadMatch !== null) {
      return requireMethod(request, ['GET'], () =>
        routeReadWindow(
          dependencies,
          env,
          decodeURIComponent(windowReadMatch[1]!),
        ).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error)))),
      )
    }

    return undefined
  },
})
