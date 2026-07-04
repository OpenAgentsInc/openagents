/**
 * Operator route module for the partner payout ledger (#4986, lane WS-E).
 *
 * Thin admin-gated HTTP surface over `partner-payout-ledger.ts`
 * (`transitionPartnerPayout` / `readCurrentPartnerPayout` /
 * `projectPartnerPayout`). Modeled directly on
 * `site-referral-payout-ledger-routes.ts` (admin-token guarded transitions
 * endpoint) and the `autopilot-decision-routes.ts` factory/route-match shape.
 *
 * Endpoints:
 *   POST /api/operator/partners/payout-ledger/:payoutRef/transitions
 *     - admin-gated; decodes { action, idempotencyKey, evidenceRefs?,
 *       stateReasonRef? } and calls `transitionPartnerPayout`. Idempotent on
 *       `idempotencyKey` (the core dedupes); replays return the same entry.
 *       action ∈ approve_dispatch | mark_dispatched | mark_failed |
 *       mark_settled | refuse | reverse.
 *   GET  /api/operator/partners/payout-ledger/:payoutRef
 *     - admin-gated; returns the current `projectPartnerPayout(...)` projection
 *       (or 404 when the ref is unknown).
 *   POST /api/operator/partners/payout-ledger/:payoutRef/dispatch
 *     - admin-gated; readiness-gated; drives a sats-denominated row through
 *       approved -> dispatched -> settled via an injected adapter that returns
 *       a public-safe `receipt.partner_payout.*` evidence ref.
 *
 * Authority boundary: ledger state is NOT spendable value. Settlement requires
 * operator-gated dispatch plus public-safe settlement evidence refs, enforced
 * inside the core module (`mark_settled` rejects without evidenceRefs).
 */
import { Effect, Match as M, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { makeTreasuryDatabaseForEnv } from './treasury-domain-store'
import { decodeUnknownWithSchema } from './json-boundary'
import {
  type PartnerPayoutDispatchDependencies,
  PartnerPayoutDispatchError,
  dispatchPartnerPayoutSettlement,
} from './partner-payout-dispatch'
import {
  PartnerPayoutLedgerStorageError,
  PartnerPayoutLedgerValidationError,
  projectPartnerPayout,
  readCurrentPartnerPayout,
  transitionPartnerPayout,
} from './partner-payout-ledger'
import { openAgentsDatabase } from './runtime'

type HttpResponse = globalThis.Response

type PartnerPayoutLedgerEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type PartnerPayoutLedgerRouteDependencies<
  Bindings extends PartnerPayoutLedgerEnv,
> = Readonly<{
  dispatchDependencies: PartnerPayoutDispatchDependencies
  nowIso: () => string
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
}>

class PartnerPayoutLedgerUnauthorized extends S.TaggedErrorClass<PartnerPayoutLedgerUnauthorized>()(
  'PartnerPayoutLedgerUnauthorized',
  {},
) {}

class PartnerPayoutLedgerBadRequest extends S.TaggedErrorClass<PartnerPayoutLedgerBadRequest>()(
  'PartnerPayoutLedgerBadRequest',
  {
    reason: S.String,
  },
) {}

class PartnerPayoutLedgerNotFound extends S.TaggedErrorClass<PartnerPayoutLedgerNotFound>()(
  'PartnerPayoutLedgerNotFound',
  {},
) {}

type PartnerPayoutLedgerRouteError =
  | PartnerPayoutLedgerBadRequest
  | PartnerPayoutLedgerNotFound
  | PartnerPayoutLedgerStorageError
  | PartnerPayoutLedgerUnauthorized
  | PartnerPayoutLedgerValidationError

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const TransitionRequest = S.Struct({
  action: S.Literals([
    'approve_dispatch',
    'mark_dispatched',
    'mark_failed',
    'mark_settled',
    'refuse',
    'reverse',
  ]),
  evidenceRefs: S.optionalKey(S.Array(S.Trim.check(S.isMaxLength(300)))),
  idempotencyKey: S.Trim.check(S.isMinLength(1), S.isMaxLength(220)),
  stateReasonRef: S.optionalKey(S.NullOr(S.Trim.check(S.isMaxLength(300)))),
})

const routeErrorResponse = (
  error: PartnerPayoutLedgerRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      PartnerPayoutLedgerBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      PartnerPayoutLedgerNotFound: () =>
        noStoreJsonResponse(
          { error: 'partner_payout_not_found' },
          { status: 404 },
        ),
      PartnerPayoutLedgerStorageError: () =>
        noStoreJsonResponse(
          { error: 'partner_payout_ledger_storage_error' },
          { status: 500 },
        ),
      PartnerPayoutLedgerUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
      PartnerPayoutLedgerValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'partner_payout_invalid_transition', reason },
          { status: 409 },
        ),
    }),
    M.exhaustive,
  )

const runRoute = (
  effect: Effect.Effect<HttpResponse, PartnerPayoutLedgerRouteError>,
): Effect.Effect<HttpResponse> =>
  effect.pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const requireAdmin = <Bindings extends PartnerPayoutLedgerEnv>(
  dependencies: PartnerPayoutLedgerRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<void, PartnerPayoutLedgerRouteError> =>
  Effect.gen(function* () {
    const authorized = yield* Effect.tryPromise({
      catch: () => new PartnerPayoutLedgerUnauthorized({}),
      try: () => dependencies.requireAdminApiToken(request, env),
    })

    if (!authorized) {
      return yield* new PartnerPayoutLedgerUnauthorized({})
    }
  })

const transitionRoute = <Bindings extends PartnerPayoutLedgerEnv>(
  dependencies: PartnerPayoutLedgerRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  payoutRef: string,
): Effect.Effect<HttpResponse> =>
  runRoute(
    Effect.gen(function* () {
      if (request.method !== 'POST') {
        return methodNotAllowed(['POST'])
      }

      yield* requireAdmin(dependencies, request, env)

      const parsed = yield* Effect.tryPromise({
        catch: error =>
          new PartnerPayoutLedgerBadRequest({ reason: errorMessage(error) }),
        try: async () =>
          decodeUnknownWithSchema(
            TransitionRequest,
            await request.json().catch(() => ({})),
          ),
      })
      const payout = yield* Effect.tryPromise({
        catch: error =>
          error instanceof PartnerPayoutLedgerValidationError ||
          error instanceof PartnerPayoutLedgerStorageError
            ? error
            : new PartnerPayoutLedgerStorageError({
                error,
                operation: 'partnerPayoutLedger.transitionRoute',
              }),
        try: () =>
          transitionPartnerPayout(makeTreasuryDatabaseForEnv(env), {
            action: parsed.action,
            idempotencyKey: parsed.idempotencyKey,
            nowIso: dependencies.nowIso(),
            payoutRef,
            ...(parsed.evidenceRefs === undefined
              ? {}
              : { evidenceRefs: parsed.evidenceRefs }),
            ...(parsed.stateReasonRef === undefined
              ? {}
              : { stateReasonRef: parsed.stateReasonRef }),
          }),
      })

      return noStoreJsonResponse({ payout: projectPartnerPayout(payout) })
    }),
  )

const dispatchRoute = <Bindings extends PartnerPayoutLedgerEnv>(
  dependencies: PartnerPayoutLedgerRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  payoutRef: string,
): Effect.Effect<HttpResponse> =>
  runRoute(
    Effect.gen(function* () {
      if (request.method !== 'POST') {
        return methodNotAllowed(['POST'])
      }

      yield* requireAdmin(dependencies, request, env)

      const outcome = yield* Effect.tryPromise({
        catch: error =>
          error instanceof PartnerPayoutLedgerValidationError ||
          error instanceof PartnerPayoutLedgerStorageError
            ? error
            : error instanceof PartnerPayoutDispatchError
              ? new PartnerPayoutLedgerStorageError({
                  error,
                  operation: 'partnerPayoutLedger.dispatchRoute.adapter',
                })
              : new PartnerPayoutLedgerStorageError({
                  error,
                  operation: 'partnerPayoutLedger.dispatchRoute',
                }),
        try: () =>
          dispatchPartnerPayoutSettlement(
            makeTreasuryDatabaseForEnv(env),
            dependencies.dispatchDependencies,
            { payoutRef },
          ),
      })

      return noStoreJsonResponse({
        dispatch:
          outcome._tag === 'settled'
            ? {
                _tag: outcome._tag,
                amountSats: outcome.entry.amount,
                payoutRef: outcome.entry.payoutRef,
                receiptRef: outcome.receiptRef,
                state: outcome.entry.state,
              }
            : outcome._tag === 'already_settled'
              ? {
                  _tag: outcome._tag,
                  payoutRef: outcome.entry.payoutRef,
                  state: outcome.entry.state,
                }
              : {
                  _tag: outcome._tag,
                  reasonRef: outcome.reasonRef,
                  ...(outcome.entry === null
                    ? {}
                    : {
                        payoutRef: outcome.entry.payoutRef,
                        state: outcome.entry.state,
                      }),
                },
      })
    }),
  )

const projectionRoute = <Bindings extends PartnerPayoutLedgerEnv>(
  dependencies: PartnerPayoutLedgerRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  payoutRef: string,
): Effect.Effect<HttpResponse> =>
  runRoute(
    Effect.gen(function* () {
      if (request.method !== 'GET') {
        return methodNotAllowed(['GET'])
      }

      yield* requireAdmin(dependencies, request, env)

      const current = yield* Effect.tryPromise({
        catch: error =>
          error instanceof PartnerPayoutLedgerStorageError
            ? error
            : new PartnerPayoutLedgerStorageError({
                error,
                operation: 'partnerPayoutLedger.projectionRoute',
              }),
        try: () => readCurrentPartnerPayout(openAgentsDatabase(env), payoutRef),
      })

      if (current === null) {
        return yield* new PartnerPayoutLedgerNotFound({})
      }

      return noStoreJsonResponse({ payout: projectPartnerPayout(current) })
    }),
  )

const TRANSITIONS_PATTERN =
  /^\/api\/operator\/partners\/payout-ledger\/([^/]+)\/transitions$/
const DISPATCH_PATTERN =
  /^\/api\/operator\/partners\/payout-ledger\/([^/]+)\/dispatch$/
const PROJECTION_PATTERN =
  /^\/api\/operator\/partners\/payout-ledger\/([^/]+)$/

export const makePartnerPayoutLedgerRoutes = <
  Bindings extends PartnerPayoutLedgerEnv,
>(
  dependencies: PartnerPayoutLedgerRouteDependencies<Bindings>,
) => ({
  routePartnerPayoutLedgerRequest: (
    request: Request,
    env: Bindings,
    _ctx?: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    const transitionsMatch = TRANSITIONS_PATTERN.exec(url.pathname)

    if (transitionsMatch !== null) {
      return transitionRoute(
        dependencies,
        request,
        env,
        decodeURIComponent(transitionsMatch[1] ?? ''),
      )
    }

    const dispatchMatch = DISPATCH_PATTERN.exec(url.pathname)

    if (dispatchMatch !== null) {
      return dispatchRoute(
        dependencies,
        request,
        env,
        decodeURIComponent(dispatchMatch[1] ?? ''),
      )
    }

    const projectionMatch = PROJECTION_PATTERN.exec(url.pathname)

    if (projectionMatch !== null) {
      return projectionRoute(
        dependencies,
        request,
        env,
        decodeURIComponent(projectionMatch[1] ?? ''),
      )
    }

    return undefined
  },
})
