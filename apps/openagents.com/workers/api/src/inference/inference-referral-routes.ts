// Inference referral revshare routes (EPIC #5474 / sub-EPIC #5475, children
// #5491 dashboard + #5490 dispatch).
//
// Two surfaces, ONE router key:
//   - GET  /api/inference/referral/dashboard
//       Browser-session-scoped READ of the signed-in referrer's inference
//       revshare: referred accounts, ongoing earnings, settled receipts.
//       Public-safe (refs + sats only). Read-only.
//   - POST /api/operator/inference/referral/payout/:payoutRef/dispatch
//       Admin-gated dispatch of one accrued inference referral payout through
//       the shared RL-2 rail. Idempotent, readiness-gated (owner-armed), asset-
//       boundary enforced. Money moves ONLY when the readiness gate allows live
//       payouts AND the revenue is Bitcoin.
//
// The dispatch dependencies (adapter + readiness gate) are INJECTED by the
// Worker. On the inert / not-yet-armed path the readiness gate returns
// `livePayoutClaimAllowed: false`, so this route refuses (no money) even if
// reached — the first real inference referral payout is owner-armed.

import { Effect, Match as M, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../http/responses'
import { decodeUnknownWithSchema } from '../json-boundary'
import { openAgentsDatabase } from '../runtime'
import {
  type SiteReferralPayoutDispatchDependencies,
  SiteReferralPayoutDispatchError,
} from '../site-referral-payout-dispatch'
import {
  SiteReferralPayoutLedgerStorageError,
  SiteReferralPayoutLedgerValidationError,
} from '../site-referral-payout-ledger'
import { readInferenceReferralDashboard } from './inference-referral-dashboard'
import { dispatchInferenceReferralPayout } from './inference-referral-dispatch'

type HttpResponse = globalThis.Response

type InferenceReferralEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type InferenceReferralSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

export type InferenceReferralRouteDependencies<
  Session extends InferenceReferralSession,
  Bindings extends InferenceReferralEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  // Dispatch dependencies (adapter + readiness gate + clock). The Worker injects
  // the readiness-gated MDK/Spark rail; tests inject a mock adapter + a gate that
  // is false (no live payout).
  dispatchDependencies: SiteReferralPayoutDispatchDependencies
}>

class InferenceReferralUnauthorized extends S.TaggedErrorClass<InferenceReferralUnauthorized>()(
  'InferenceReferralUnauthorized',
  {},
) {}

class InferenceReferralSessionError extends S.TaggedErrorClass<InferenceReferralSessionError>()(
  'InferenceReferralSessionError',
  { error: S.Defect },
) {}

class InferenceReferralBadRequest extends S.TaggedErrorClass<InferenceReferralBadRequest>()(
  'InferenceReferralBadRequest',
  { reason: S.String },
) {}

class InferenceReferralStorageError extends S.TaggedErrorClass<InferenceReferralStorageError>()(
  'InferenceReferralStorageError',
  { error: S.Defect, operation: S.String },
) {}

type InferenceReferralRouteError =
  | InferenceReferralBadRequest
  | InferenceReferralSessionError
  | InferenceReferralStorageError
  | InferenceReferralUnauthorized
  | SiteReferralPayoutLedgerStorageError
  | SiteReferralPayoutLedgerValidationError

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const routeErrorResponse = (error: InferenceReferralRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      InferenceReferralBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      InferenceReferralSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      InferenceReferralStorageError: () =>
        noStoreJsonResponse(
          { error: 'inference_referral_storage_error' },
          { status: 500 },
        ),
      InferenceReferralUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
      SiteReferralPayoutLedgerStorageError: () =>
        noStoreJsonResponse(
          { error: 'site_referral_payout_ledger_storage_error' },
          { status: 500 },
        ),
      SiteReferralPayoutLedgerValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'site_referral_payout_invalid_transition', reason },
          { status: 409 },
        ),
    }),
    M.exhaustive,
  )

const runRoute = (
  request: Request,
  allowedMethods: readonly string[],
  effect: Effect.Effect<HttpResponse, InferenceReferralRouteError>,
): Effect.Effect<HttpResponse> =>
  allowedMethods.includes(request.method)
    ? effect.pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    : Effect.succeed(methodNotAllowed([...allowedMethods]))

const DispatchRequest = S.Struct({
  revenueAsset: S.Literals(['bitcoin', 'credit', 'usd']),
})

const DISPATCH_PATH =
  /^\/api\/operator\/inference\/referral\/payout\/([^/]+)\/dispatch$/

export const makeInferenceReferralRoutes = <
  Session extends InferenceReferralSession,
  Bindings extends InferenceReferralEnv,
>(
  dependencies: InferenceReferralRouteDependencies<Session, Bindings>,
) => {
  const dashboard = (request: Request, env: Bindings, ctx: ExecutionContext) =>
    runRoute(
      request,
      ['GET'],
      Effect.gen(function* () {
        const session = yield* Effect.tryPromise({
          catch: error => new InferenceReferralSessionError({ error }),
          try: () => dependencies.requireBrowserSession(request, env, ctx),
        })

        if (session === undefined) {
          return yield* new InferenceReferralUnauthorized({})
        }

        const referralDashboard = yield* Effect.tryPromise({
          catch: error =>
            new InferenceReferralStorageError({
              error,
              operation: 'inferenceReferral.dashboard',
            }),
          try: () =>
            readInferenceReferralDashboard(
              openAgentsDatabase(env),
              session.user.userId,
            ),
        })

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ inferenceReferralDashboard: referralDashboard }),
          session,
        )
      }),
    )

  const dispatch = (
    request: Request,
    env: Bindings,
    payoutRef: string,
  ) =>
    runRoute(
      request,
      ['POST'],
      Effect.gen(function* () {
        const authorized = yield* Effect.tryPromise({
          catch: () => new InferenceReferralUnauthorized({}),
          try: () => dependencies.requireAdminApiToken(request, env),
        })

        if (!authorized) {
          return yield* new InferenceReferralUnauthorized({})
        }

        const parsed = yield* Effect.tryPromise({
          catch: error =>
            new InferenceReferralBadRequest({ reason: errorMessage(error) }),
          try: async () =>
            decodeUnknownWithSchema(
              DispatchRequest,
              await request.json().catch(() => ({})),
            ),
        })

        const outcome = yield* Effect.tryPromise({
          catch: error =>
            error instanceof SiteReferralPayoutLedgerValidationError ||
            error instanceof SiteReferralPayoutLedgerStorageError
              ? error
              : error instanceof SiteReferralPayoutDispatchError
                ? new InferenceReferralStorageError({
                    error,
                    operation: 'inferenceReferral.dispatch.adapter',
                  })
                : new InferenceReferralStorageError({
                    error,
                    operation: 'inferenceReferral.dispatch',
                  }),
          try: () =>
            dispatchInferenceReferralPayout(
              openAgentsDatabase(env),
              dependencies.dispatchDependencies,
              { payoutRef, revenueAsset: parsed.revenueAsset },
            ),
        })

        // Public-safe dispatch outcome: tag + payoutRef + (refused) reasonRef or
        // (settled) receiptRef. Never amounts beyond the ledger's own sats, never
        // payment material.
        return noStoreJsonResponse({
          dispatch:
            outcome._tag === 'settled'
              ? {
                  _tag: outcome._tag,
                  amountSats: outcome.entry.amountSats,
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
                      : { state: outcome.entry.state }),
                  },
        })
      }),
    )

  return {
    routeInferenceReferralRequest: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)

      if (url.pathname === '/api/inference/referral/dashboard') {
        return dashboard(request, env, ctx)
      }

      const dispatchMatch = DISPATCH_PATH.exec(url.pathname)
      if (dispatchMatch !== null) {
        return dispatch(request, env, decodeURIComponent(dispatchMatch[1] ?? ''))
      }

      return undefined
    },
  }
}
