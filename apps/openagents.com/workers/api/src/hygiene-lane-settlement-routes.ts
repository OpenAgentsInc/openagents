import { Effect, Match as M, Schema as S } from 'effect'

import { type DebtReceiptSettlementProjection } from './debt-receipt-policy'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
import {
  HygieneLaneSettlementRequest,
  HygieneLaneVerificationClass,
  buildHygieneLaneSettlement,
  decideHygieneLaneSettlement,
} from './hygiene-lane-settlement'
import { decodeUnknownWithSchema, readJsonObject } from './json-boundary'
import type { NexusTreasuryPayoutLedgerStore } from './nexus-treasury-payout-ledger'
import { currentIsoTimestamp } from './runtime-primitives'
import { readTassadarRealSettlementGate } from './tassadar-run-settlement-gate'
import {
  type RealRunSettlementDispatchResolvers,
  dispatchRealRunSettlementCore,
} from './training-run-window-routes'
import {
  TrainingAuthorityStoreError,
  trainingAuthorityStoreErrorFromUnknown,
} from './training-run-window-authority'

/**
 * Honest hygiene-lane settlement DISPATCH route (openagents #5372, EPIC #5335).
 *
 * `POST /api/hygiene-lane/settlement-receipt` settles ONE merged,
 * benchmark-verified hygiene debt receipt to the contributor's registered Spark
 * payout target. It REUSES the proven #5232 Spark treasury money rail
 * (`dispatchRealRunSettlementCore`: intent -> attempt -> reconcile ->
 * `settlement_recorded`) and the SAME owner gate
 * (`OPENAGENTS_REAL_SETTLEMENT_GATE`), but with an HONEST verification basis:
 *
 *   - Hygiene PRs are verified by TESTS + REVIEWER ACCEPTANCE + the merged debt
 *     receipt (#5340), NOT by exact trace replay. This route therefore never
 *     touches the Tassadar settle endpoint (which requires a Verified
 *     `exact_trace_replay` challenge and a window lease) and never emits an
 *     `exact_trace_replay` verdict. The receipt projection states the
 *     `hygiene_merged_reviewed` basis instead.
 *
 * It is fail-closed at every step:
 *   - admin-only,
 *   - the debt-receipt projection (resolved server-side by `debtReceiptKeyRef`)
 *     must be `payable`; absent/blocked/duplicate -> no settle,
 *   - the interim amount must be payable (> 0, <= 100),
 *   - the owner gate must authorize this run/contributor/amount,
 *   - the contributor must have a registered Spark destination (the destination
 *     resolver inside the dispatch core fails closed when absent),
 *   - one settlement per `DebtReceiptKey` (#5340) AND idempotent on
 *     `idempotencyRef` (a retry pays AT MOST once — the dispatch core
 *     short-circuits on the deterministic receipt ref).
 *
 * Real-vs-simulation is honest: with the gate OFF (the default everywhere) the
 * route records the simulation chain (`moneyMovement:'none'`) and reports
 * `realBitcoinMoved:false`. Only an armed gate authorizes the real Spark branch.
 */

type HttpResponse = globalThis.Response

type HygieneLaneSettlementRouteEnv = Readonly<Record<string, unknown>>

class HygieneLaneSettlementUnauthorized extends S.TaggedErrorClass<HygieneLaneSettlementUnauthorized>()(
  'HygieneLaneSettlementUnauthorized',
  {},
) {}

type HygieneLaneSettlementRouteError =
  | TrainingAuthorityStoreError
  | HygieneLaneSettlementUnauthorized

export type HygieneLaneSettlementRouteDependencies<Bindings> = Readonly<{
  // The treasury payout ledger (reads existing receipts for idempotency, writes
  // the run-tied settlement chain). Same store the Tassadar rail uses.
  makePayoutLedgerStore: (env: Bindings) => NexusTreasuryPayoutLedgerStore
  // Resolve the live debt-receipt settlement projection for a DebtReceiptKey
  // ref. Fail-closed: an undefined result means "no payable debt receipt" and
  // the route does not settle. The projection is the SOURCE OF TRUTH for
  // payability — the operator cannot assert it through the request body.
  resolveDebtReceiptProjection: (
    env: Bindings,
    debtReceiptKeyRef: string,
  ) => Promise<DebtReceiptSettlementProjection | undefined>
  // REAL-settlement dispatch wiring (openagents #5232). All optional: the real
  // branch is unreachable unless the owner arms the gate, and then it requires
  // these to be wired. INERT by default.
  makeSettlementPaymentAuthority?: RealRunSettlementDispatchResolvers<Bindings>['makeSettlementPaymentAuthority']
  readSettlementWalletReadiness?: RealRunSettlementDispatchResolvers<Bindings>['readSettlementWalletReadiness']
  resolveSettlementPayoutDestination?: RealRunSettlementDispatchResolvers<Bindings>['resolveSettlementPayoutDestination']
  nowIso?: () => string
  requireAdminApiToken?: (request: Request, env: Bindings) => Promise<boolean>
}>

const routeErrorResponse = (
  error: HygieneLaneSettlementRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      HygieneLaneSettlementUnauthorized: () => unauthorized(),
      TrainingAuthorityStoreError: storeError =>
        noStoreJsonResponse(
          {
            error: `hygiene_lane_settlement_${storeError.kind}`,
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
    }),
    M.exhaustive,
  )

const requireAdmin = <Bindings extends HygieneLaneSettlementRouteEnv>(
  dependencies: HygieneLaneSettlementRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<void, HygieneLaneSettlementUnauthorized> =>
  Effect.tryPromise({
    catch: () => new HygieneLaneSettlementUnauthorized({}),
    try: () =>
      dependencies.requireAdminApiToken?.(request, env) ??
      Promise.resolve(false),
  }).pipe(
    Effect.flatMap(isAdmin =>
      isAdmin
        ? Effect.void
        : Effect.fail(new HygieneLaneSettlementUnauthorized({})),
    ),
  )

const routeHygieneLaneSettlement = <
  Bindings extends HygieneLaneSettlementRouteEnv,
>(
  dependencies: HygieneLaneSettlementRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, HygieneLaneSettlementRouteError> =>
  Effect.gen(function* () {
    yield* requireAdmin(dependencies, request, env)

    const body = yield* Effect.tryPromise({
      catch: error =>
        new TrainingAuthorityStoreError({
          kind: 'validation_error',
          reason: error instanceof Error ? error.message : String(error),
        }),
      try: async () =>
        decodeUnknownWithSchema(
          HygieneLaneSettlementRequest,
          await readJsonObject(request),
        ),
    })

    const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()

    // Resolve the live debt-receipt projection by its key. Fail-closed: a
    // missing projection is NOT payable. The operator cannot assert payability
    // through the request body; the policy state machine is the authority.
    const debtReceiptProjection = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () =>
        dependencies.resolveDebtReceiptProjection(env, body.debtReceiptKeyRef),
    })

    if (debtReceiptProjection === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'hygiene_lane_settlement_blocked:debt_receipt_not_found',
      })
    }

    // The supplied key ref must match the resolved projection's key — a
    // mismatch means the operator referenced a different receipt than the one
    // the policy resolved. Fail closed.
    if (
      debtReceiptProjection.debtReceiptKey !== null &&
      debtReceiptProjection.debtReceiptKey !== body.debtReceiptKeyRef
    ) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'validation_error',
        reason: 'hygiene_lane_settlement_blocked:debt_receipt_key_mismatch',
      })
    }

    const gate = readTassadarRealSettlementGate(env)
    const requestedAdapterKind = body.adapterKind ?? 'simulation'

    // The single gated decision: run-ref + payable debt receipt + payable
    // amount + gate authority (run/contributor/amount). Pure and fail-closed.
    const decision = decideHygieneLaneSettlement({
      contributorRef: body.contributorRef,
      debtReceiptProjection,
      gate,
      requestedAdapterKind,
      signals: body.signals,
      trainingRunRef: body.trainingRunRef,
    })

    // `gate_decision_blocked` is NOT an error: it is the normal,
    // real-vs-simulation HONEST fallback. With the owner gate OFF (the default
    // everywhere) the decision is unauthorized-for-real, and we record the
    // simulation chain below (moneyMovement 'none', realBitcoinMoved:false).
    // Every OTHER block is a true fail-closed: the work is not settleable at all
    // (not a hygiene run, debt receipt not payable, duplicate replay, or the
    // amount was denied), so we surface a typed error and never touch money.
    if (
      decision.blockedReason !== null &&
      decision.blockedReason !== 'gate_decision_blocked'
    ) {
      return yield* new TrainingAuthorityStoreError({
        kind:
          decision.blockedReason === 'debt_receipt_not_payable' ||
          decision.blockedReason === 'duplicate_replay'
            ? 'conflict'
            : 'validation_error',
        reason: `hygiene_lane_settlement_blocked:${decision.blockedReason}`,
      })
    }

    // Build the settlement ledger chain with the HONEST hygiene basis. On the
    // simulation branch the resolved adapter is `simulation` (moneyMovement
    // 'none'); on the real branch it is `spark_treasury` (moneyMovement
    // 'real_bitcoin'). The resolved adapter — not the raw request — drives the
    // builder, so the simulation path never claims real money.
    const settlement = buildHygieneLaneSettlement({
      adapterKind: decision.gateDecision?.adapterKind ?? 'simulation',
      amountSats: decision.amount.payoutSats,
      contributorRef: body.contributorRef,
      debtReceiptKeyRef: body.debtReceiptKeyRef,
      idempotencyRef: body.idempotencyRef,
      mergedPrRef: body.mergedPrRef,
      nowIso,
      operatorApprovalRef: body.operatorApprovalRef,
      payoutTargetApprovalRef: body.payoutTargetApprovalRef,
      payoutTargetRef: body.payoutTargetRef,
      reviewerAcceptanceRef: body.reviewerAcceptanceRef,
      trainingRunRef: body.trainingRunRef,
    })

    const ledger = dependencies.makePayoutLedgerStore(env)

    if (decision.realAuthorized) {
      // REAL Bitcoin branch. Reuse the proven receipt-first, idempotent Spark
      // dispatch core (intent -> dispatch -> reconcile -> settlement_recorded).
      // A retry on the same idempotencyRef pays AT MOST once.
      const {
        makeSettlementPaymentAuthority,
        readSettlementWalletReadiness,
        resolveSettlementPayoutDestination,
      } = dependencies

      if (
        makeSettlementPaymentAuthority === undefined ||
        readSettlementWalletReadiness === undefined ||
        resolveSettlementPayoutDestination === undefined
      ) {
        return yield* new TrainingAuthorityStoreError({
          kind: 'storage_error',
          reason:
            'hygiene_lane_settlement_blocked:real_authorized_but_payout_authority_not_configured',
        })
      }

      yield* dispatchRealRunSettlementCore<Bindings>(
        {
          env,
          makeSettlementPaymentAuthority,
          readSettlementWalletReadiness,
          resolveSettlementPayoutDestination,
        },
        {
          contributorRef: settlement.contributorRef,
          ledger,
          settlement,
        },
      ).pipe(
        // The dispatch core's error type is the Tassadar route union (which
        // includes an unauthorized variant it never actually raises here).
        // Normalize any escape into a typed store error so this route's error
        // union stays `TrainingAuthorityStoreError | unauthorized`.
        Effect.mapError(error =>
          error._tag === 'TrainingAuthorityStoreError'
            ? error
            : new TrainingAuthorityStoreError({
                kind: 'storage_error',
                reason: 'hygiene_lane_settlement_blocked:dispatch_failed',
              }),
        ),
      )
    } else {
      // Honest SIMULATION chain (gate OFF, the default). Records the same ledger
      // shape with moneyMovement 'none' so the public derivation yields
      // realBitcoinMoved:false. Idempotent: skip if the receipt already exists.
      yield* Effect.tryPromise({
        catch: trainingAuthorityStoreErrorFromUnknown,
        try: async () => {
          const existing = await ledger.readPaymentAuthorityReceiptByRef(
            settlement.settlementReceiptRef,
          )

          if (existing !== undefined) {
            return
          }

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

    return noStoreJsonResponse({
      settlement: {
        amountSats: settlement.amountSats,
        contributorRef: settlement.contributorRef,
        debtReceiptKey: decision.debtReceiptKey,
        moneyMovement: decision.realAuthorized ? 'real_bitcoin' : 'none',
        publicProjectionRefs: decision.publicProjectionRefs,
        realAuthorized: decision.realAuthorized,
        settlementReceiptRef: settlement.settlementReceiptRef,
        verificationBasis: HygieneLaneVerificationClass,
      },
    })
  })

export const makeHygieneLaneSettlementRoutes = <
  Bindings extends HygieneLaneSettlementRouteEnv,
>(
  dependencies: HygieneLaneSettlementRouteDependencies<Bindings>,
) => ({
  routeHygieneLaneSettlementRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname !== '/api/hygiene-lane/settlement-receipt') {
      return undefined
    }

    if (request.method !== 'POST') {
      return Effect.succeed(methodNotAllowed(['POST']))
    }

    return routeHygieneLaneSettlement(dependencies, request, env).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  },
})
