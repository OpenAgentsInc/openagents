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
 *
 * Authority boundary: ledger state is NOT spendable value. Settlement requires
 * operator-gated dispatch plus public-safe settlement evidence refs, enforced
 * inside the core module (`mark_settled` rejects without evidenceRefs).
 *
 * ============================================================================
 * COORDINATOR WIRING (deferred integration — do NOT wire from this lane)
 * ----------------------------------------------------------------------------
 * This lane intentionally does NOT touch the shared `index.ts` /
 * `worker-routes.ts` files. The coordinator integrating this lane must:
 *
 * 1. Construct the routes in `index.ts` alongside the referral payout routes,
 *    reusing the existing `requireAdminApiToken` and `currentIsoTimestamp`:
 *
 *      import { makePartnerPayoutLedgerRoutes }
 *        from './partner-payout-ledger-routes'
 *
 *      const partnerPayoutLedgerRoutes = makePartnerPayoutLedgerRoutes({
 *        nowIso: currentIsoTimestamp,
 *        requireAdminApiToken,
 *      })
 *
 * 2. Expose the route on the worker-routes dependency object (next to
 *    `routeSiteReferralPayoutLedgerRequest`):
 *
 *      routePartnerPayoutLedgerRequest:
 *        partnerPayoutLedgerRoutes.routePartnerPayoutLedgerRequest,
 *
 * 3. Add the field to the `worker-routes.ts` dependency type as an
 *    `OptionalEffectRoute`, then chain it inside `routeOmniRequest` right
 *    after the referral payout block:
 *
 *      const partnerPayoutLedgerResponse =
 *        dependencies.routePartnerPayoutLedgerRequest(request, env, ctx)
 *
 *      if (partnerPayoutLedgerResponse !== undefined) {
 *        return yield* partnerPayoutLedgerResponse
 *      }
 *
 *    (Order is not significant; the path prefixes do not collide with the
 *    referral `/api/operator/sites/referrals/payout-ledger/...` routes.)
 *
 * 4. Apply migration `0184_partner_payout_ledger.sql` before serving (see the
 *    core module's wiring notes).
 * ============================================================================
 */
import { Effect, Match as M, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { decodeUnknownWithSchema } from './json-boundary'
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
          transitionPartnerPayout(openAgentsDatabase(env), {
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
