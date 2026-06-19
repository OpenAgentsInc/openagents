import { Effect, Match as M, Schema as S } from 'effect'

import { sha256Hex } from './agent-registration'
import type { AssetBoundaryAsset } from './asset-bitcoin-boundary'
import {
  FirmupExecutedVerificationVerdict,
  FirmupSettlementUnsafe,
  FirmupSettlementVerificationClass,
  buildFirmupBitcoinSettlement,
  decideFirmupBitcoinSettlement,
} from './firmup-bitcoin-settlement'
import {
  methodNotAllowed,
  noStoreJsonResponse,
  unauthorized,
} from './http/responses'
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
 * Firm-up escrow -> real Bitcoin settlement DISPATCH route (openagents #5459,
 * EPIC #5457).
 *
 * `POST /api/firmup-lane/settlement-receipt` settles ONE firmed-up,
 * EXECUTED-verified labor job to the worker's registered Spark payout target. It
 * REUSES the proven #5232 Spark treasury money rail
 * (`dispatchRealRunSettlementCore`: intent -> attempt -> reconcile ->
 * `settlement_recorded`) and the SAME owner gate
 * (`OPENAGENTS_REAL_SETTLEMENT_GATE`) the Tassadar + hygiene lanes use.
 *
 * This is the wire §2H called out as missing: today the NIP-90 firm-up escrow
 * (`labor-escrow.ts`) releases INTERNAL msat credits and gates release on a
 * caller-supplied attestation string. This route turns a firmed-up job into REAL
 * Bitcoin to the worker against an EXECUTED verification verdict (the Pylon-side
 * replay/verify substrate), not a manual attestation.
 *
 * It is fail-closed at every step:
 *   - admin-only,
 *   - the escrow (resolved server-side by `escrowRef`) must be a real, reserved
 *     firm-up escrow with a recorded result + accepted offer; the operator
 *     cannot assert settleability through the request body,
 *   - the verification verdict must be EXECUTED + `verified` (carry an
 *     executed-trace digest) — a manual attestation is refused,
 *   - the worker (provider) must differ from the validator,
 *   - the run-ref must be a firm-up lane run-ref,
 *   - the owner gate must authorize this run/contributor/amount,
 *   - the worker must have a registered Spark destination (the destination
 *     resolver inside the dispatch core fails closed when absent),
 *   - idempotent on the escrow release (a retry pays AT MOST once — the dispatch
 *     core short-circuits on the deterministic receipt ref).
 *
 * Real-vs-simulation is honest: with the gate OFF (the default everywhere) the
 * route records the simulation chain (`moneyMovement:'none'`) and reports
 * `realBitcoinMoved:false`. Only an armed gate authorizes the real Spark branch.
 * No firm-up run-ref is added to the gate by this change; the first real firm-up
 * payout is a deliberate prod event the owner arms separately.
 */

type HttpResponse = globalThis.Response

type FirmupSettlementRouteEnv = Readonly<Record<string, unknown>>

class FirmupSettlementUnauthorized extends S.TaggedErrorClass<FirmupSettlementUnauthorized>()(
  'FirmupSettlementUnauthorized',
  {},
) {}

type FirmupSettlementRouteError =
  | TrainingAuthorityStoreError
  | FirmupSettlementUnauthorized

/**
 * The server-resolved, settleable firm-up escrow projection. This is the SOURCE
 * OF TRUTH for whether a firm-up job may settle: it is read server-side from the
 * escrow + acceptance + result rows, never asserted by the request body. Every
 * field is a public-safe ref or a bounded integer.
 */
export type FirmupSettleableEscrowProjection = Readonly<{
  // The amount the firmed-up escrow holds, in sats.
  amountSats: number
  // The escrow public ref this settlement releases.
  escrowRef: string
  // The worker (provider) actor whose registered Spark target is paid.
  providerActorRef: string
  // RL-3 (#5460): the asset the escrow's qualifying REVENUE was sourced in,
  // resolved server-side from the escrow funding (never asserted by the request
  // body). A firm-up labor escrow is Bitcoin-funded, so this defaults to
  // `bitcoin` when omitted; a credit/USD/free-funded escrow can never settle to
  // withdrawable Bitcoin (the shared boundary guard refuses it).
  revenueAsset?: AssetBoundaryAsset | undefined
  // The verification command ref the job declared (must match the verdict).
  verificationCommandRef: string
  // The work request public ref.
  workRequestRef: string
}>

export type FirmupSettlementRouteDependencies<Bindings> = Readonly<{
  // The treasury payout ledger (reads existing receipts for idempotency, writes
  // the settlement chain). Same store the Tassadar / hygiene rails use.
  makePayoutLedgerStore: (env: Bindings) => NexusTreasuryPayoutLedgerStore
  // Resolve the live, settleable firm-up escrow projection for an escrow ref.
  // Fail-closed: an undefined result means "no settleable firm-up escrow" and
  // the route does not settle. This is the SOURCE OF TRUTH — the operator cannot
  // assert settleability through the request body.
  resolveSettleableEscrow: (
    env: Bindings,
    escrowRef: string,
  ) => Promise<FirmupSettleableEscrowProjection | undefined>
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
  error: FirmupSettlementRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      FirmupSettlementUnauthorized: () => unauthorized(),
      TrainingAuthorityStoreError: storeError =>
        noStoreJsonResponse(
          {
            error: `firmup_lane_settlement_${storeError.kind}`,
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

const FirmupSettlementRequestRef = S.Trim.check(
  S.isNonEmpty(),
  S.isMaxLength(261),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/#-]{0,260}$/),
)

/**
 * Admin request to settle ONE firmed-up, EXECUTED-verified labor job (#5459).
 * Every field is a public-safe ref or the executed verdict struct. The escrow's
 * amount, worker, and work request are resolved server-side from `escrowRef`;
 * the request supplies only the executed verdict + the operator/payout-target
 * approval refs + the firm-up run-ref the gate must allowlist.
 */
export const FirmupLaneSettlementRequest = S.Struct({
  // The single allowed real adapter; defaults to simulation (honest fail-closed).
  adapterKind: S.optionalKey(
    S.Literals([
      'hosted_mdk',
      'legacy_nexus_import',
      'mdk_agent_wallet',
      'simulation',
      'spark_treasury',
    ]),
  ),
  // The firm-up escrow public ref this settlement releases.
  escrowRef: FirmupSettlementRequestRef,
  // Operator approval ref (the human who authorized this settle).
  operatorApprovalRef: FirmupSettlementRequestRef,
  // Payout-target approval ref + payout-target ref (owner-scoped). The raw
  // destination is NEVER in the request; only these refs are.
  payoutTargetApprovalRef: FirmupSettlementRequestRef,
  payoutTargetRef: FirmupSettlementRequestRef,
  // The firm-up lane run-ref the gate must allowlist (run.firmup.lane.YYYYMMDD).
  trainingRunRef: FirmupSettlementRequestRef,
  // The EXECUTED verification verdict (the trust anchor: a manual attestation
  // cannot supply the executed-trace digest this requires).
  verdict: FirmupExecutedVerificationVerdict,
})
export type FirmupLaneSettlementRequest =
  typeof FirmupLaneSettlementRequest.Type

const requireAdmin = <Bindings extends FirmupSettlementRouteEnv>(
  dependencies: FirmupSettlementRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<void, FirmupSettlementUnauthorized> =>
  Effect.tryPromise({
    catch: () => new FirmupSettlementUnauthorized({}),
    try: () =>
      dependencies.requireAdminApiToken?.(request, env) ??
      Promise.resolve(false),
  }).pipe(
    Effect.flatMap(isAdmin =>
      isAdmin
        ? Effect.void
        : Effect.fail(new FirmupSettlementUnauthorized({})),
    ),
  )

const routeFirmupLaneSettlement = <
  Bindings extends FirmupSettlementRouteEnv,
>(
  dependencies: FirmupSettlementRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse, FirmupSettlementRouteError> =>
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
          FirmupLaneSettlementRequest,
          await readJsonObject(request),
        ),
    })

    const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()

    // Resolve the live, settleable escrow projection by its ref. Fail-closed: a
    // missing projection is NOT settleable. The operator cannot assert
    // settleability through the request body; the escrow state machine is the
    // authority (reserved escrow + recorded result + accepted offer).
    const escrow = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => dependencies.resolveSettleableEscrow(env, body.escrowRef),
    })

    if (escrow === undefined) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'not_found',
        reason: 'firmup_lane_settlement_blocked:settleable_escrow_not_found',
      })
    }

    // The executed verdict's command must match the job's declared verification
    // command — a mismatch means the verdict is for a different check than the
    // firm-up job required. Fail closed.
    if (body.verdict.verificationCommandRef !== escrow.verificationCommandRef) {
      return yield* new TrainingAuthorityStoreError({
        kind: 'validation_error',
        reason:
          'firmup_lane_settlement_blocked:verification_command_ref_mismatch',
      })
    }

    const gate = readTassadarRealSettlementGate(env)
    const requestedAdapterKind = body.adapterKind ?? 'simulation'

    // The single gated decision: executed+verified verdict + worker!=validator +
    // firm-up run-ref + positive amount under cap + gate authority. Pure and
    // fail-closed. May throw FirmupSettlementUnsafe on an unsafe ref.
    const decision = yield* Effect.try({
      catch: error =>
        new TrainingAuthorityStoreError({
          kind: 'validation_error',
          reason:
            error instanceof FirmupSettlementUnsafe
              ? `firmup_lane_settlement_blocked:unsafe_ref:${error.reason}`
              : 'firmup_lane_settlement_blocked:invalid_input',
        }),
      try: () =>
        decideFirmupBitcoinSettlement({
          amountSats: escrow.amountSats,
          gate,
          providerActorRef: escrow.providerActorRef,
          requestedAdapterKind,
          // RL-3 (#5460): the boundary + no-resale guards run inside the
          // decision. The revenue asset comes from the server-resolved escrow
          // (default `bitcoin` for a Bitcoin-funded firm-up labor escrow); the
          // monetization kind is `agentic_work` (firm-up is agent labor, the
          // allowed no-resale path). A credit/USD/free-funded escrow is refused
          // by the shared boundary before any money moves.
          revenueAsset: escrow.revenueAsset ?? 'bitcoin',
          trainingRunRef: body.trainingRunRef,
          verdict: body.verdict,
        }),
    })

    // `gate_decision_blocked` is NOT an error: it is the normal,
    // real-vs-simulation HONEST fallback. With the owner gate OFF (the default
    // everywhere) the decision is unauthorized-for-real, and we record the
    // simulation chain below (moneyMovement 'none', realBitcoinMoved:false).
    // Every OTHER block is a true fail-closed: the work is not settleable at all
    // (manual attestation, rejected verification, self-pair, not a firm-up run,
    // or a bad amount), so we surface a typed error and never touch money.
    if (
      decision.blockedReason !== null &&
      decision.blockedReason !== 'gate_decision_blocked'
    ) {
      return yield* new TrainingAuthorityStoreError({
        kind:
          decision.blockedReason === 'asset_boundary_violation' ||
          decision.blockedReason === 'monetization_not_authorized' ||
          decision.blockedReason === 'not_executed_verification' ||
          decision.blockedReason === 'verification_rejected' ||
          decision.blockedReason === 'worker_validator_not_distinct'
            ? 'conflict'
            : 'validation_error',
        reason: `firmup_lane_settlement_blocked:${decision.blockedReason}`,
      })
    }

    // Build the settlement ledger chain. On the simulation branch the resolved
    // adapter is `simulation` (moneyMovement 'none'); on the real branch it is
    // `spark_treasury` (moneyMovement 'real_bitcoin'). The resolved adapter —
    // not the raw request — drives the builder, so simulation never claims real
    // money. Idempotency is keyed on the escrow release (escrowRef), so a retry
    // on the same firmed-up escrow pays AT MOST once.
    const idempotencyDigestHex = yield* Effect.tryPromise({
      catch: trainingAuthorityStoreErrorFromUnknown,
      try: () => sha256Hex(`firmup_lane_settlement:${escrow.escrowRef}`),
    })
    const settlement = buildFirmupBitcoinSettlement({
      adapterKind: decision.gateDecision?.adapterKind ?? 'simulation',
      amountSats: decision.amountSats,
      escrowRef: escrow.escrowRef,
      idempotencyDigestHex,
      nowIso,
      operatorApprovalRef: body.operatorApprovalRef,
      payoutTargetApprovalRef: body.payoutTargetApprovalRef,
      payoutTargetRef: body.payoutTargetRef,
      providerActorRef: escrow.providerActorRef,
      trainingRunRef: body.trainingRunRef,
      validatorActorRef: body.verdict.validatorActorRef,
      verdict: body.verdict,
      workRequestRef: escrow.workRequestRef,
    })

    const ledger = dependencies.makePayoutLedgerStore(env)

    if (decision.realAuthorized) {
      // REAL Bitcoin branch. Reuse the proven receipt-first, idempotent Spark
      // dispatch core (intent -> dispatch -> reconcile -> settlement_recorded).
      // A retry on the same escrow pays AT MOST once.
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
            'firmup_lane_settlement_blocked:real_authorized_but_payout_authority_not_configured',
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
        // Normalize any escape from the dispatch core's error union into a typed
        // store error so this route's error union stays
        // `TrainingAuthorityStoreError | unauthorized`.
        Effect.mapError(error =>
          error._tag === 'TrainingAuthorityStoreError'
            ? error
            : new TrainingAuthorityStoreError({
                kind: 'storage_error',
                reason: 'firmup_lane_settlement_blocked:dispatch_failed',
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
        escrowRef: escrow.escrowRef,
        moneyMovement: decision.realAuthorized ? 'real_bitcoin' : 'none',
        publicProjectionRefs: decision.publicProjectionRefs,
        realAuthorized: decision.realAuthorized,
        settlementReceiptRef: settlement.settlementReceiptRef,
        verificationBasis: FirmupSettlementVerificationClass,
      },
    })
  })

export const makeFirmupBitcoinSettlementRoutes = <
  Bindings extends FirmupSettlementRouteEnv,
>(
  dependencies: FirmupSettlementRouteDependencies<Bindings>,
) => ({
  routeFirmupLaneSettlementRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname !== '/api/firmup-lane/settlement-receipt') {
      return undefined
    }

    if (request.method !== 'POST') {
      return Effect.succeed(methodNotAllowed(['POST']))
    }

    return routeFirmupLaneSettlement(dependencies, request, env).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  },
})
