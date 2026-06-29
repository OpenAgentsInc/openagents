import { Effect, Match as M, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { decodeUnknownWithSchema } from './json-boundary'
import { openAgentsDatabase } from './runtime'
import {
  type SiteReferralPayoutDispatchDependencies,
  SiteReferralPayoutDispatchError,
  dispatchReferralPayoutSettlement,
} from './site-referral-payout-dispatch'
import {
  SiteReferralPayoutLedgerStorageError,
  SiteReferralPayoutLedgerValidationError,
  projectSiteReferralPayout,
  transitionReferralPayout,
} from './site-referral-payout-ledger'

type HttpResponse = globalThis.Response

type SiteReferralPayoutLedgerEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type SiteReferralPayoutLedgerRouteDependencies<
  Bindings extends SiteReferralPayoutLedgerEnv,
> = Readonly<{
  dispatchDependencies: SiteReferralPayoutDispatchDependencies
  nowIso: () => string
  requireAdminApiToken: (
    request: Request,
    env: Bindings,
  ) => Promise<boolean>
}>

class SiteReferralPayoutLedgerUnauthorized extends S.TaggedErrorClass<SiteReferralPayoutLedgerUnauthorized>()(
  'SiteReferralPayoutLedgerUnauthorized',
  {},
) {}

class SiteReferralPayoutLedgerBadRequest extends S.TaggedErrorClass<SiteReferralPayoutLedgerBadRequest>()(
  'SiteReferralPayoutLedgerBadRequest',
  {
    reason: S.String,
  },
) {}

type SiteReferralPayoutLedgerRouteError =
  | SiteReferralPayoutLedgerBadRequest
  | SiteReferralPayoutLedgerStorageError
  | SiteReferralPayoutLedgerUnauthorized
  | SiteReferralPayoutLedgerValidationError

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

const DispatchRequest = S.Struct({
  revenueAsset: S.Literals(['bitcoin', 'credit', 'usd']),
})

const routeErrorResponse = (
  error: SiteReferralPayoutLedgerRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      SiteReferralPayoutLedgerBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      SiteReferralPayoutLedgerStorageError: () =>
        noStoreJsonResponse(
          { error: 'site_referral_payout_ledger_storage_error' },
          { status: 500 },
        ),
      SiteReferralPayoutLedgerUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
      SiteReferralPayoutLedgerValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'site_referral_payout_invalid_transition', reason },
          { status: 409 },
        ),
    }),
    M.exhaustive,
  )

const runRoute = (
  effect: Effect.Effect<HttpResponse, SiteReferralPayoutLedgerRouteError>,
): Effect.Effect<HttpResponse> =>
  effect.pipe(
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

export const makeSiteReferralPayoutLedgerRoutes = <
  Bindings extends SiteReferralPayoutLedgerEnv,
>(
  dependencies: SiteReferralPayoutLedgerRouteDependencies<Bindings>,
) => ({
  routeSiteReferralPayoutLedgerRequest: (
    request: Request,
    env: Bindings,
    _ctx?: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    const transitionMatch =
      /^\/api\/operator\/sites\/referrals\/payout-ledger\/([^/]+)\/transitions$/.exec(
        url.pathname,
      )
    const dispatchMatch =
      /^\/api\/operator\/sites\/referrals\/payout-ledger\/([^/]+)\/dispatch$/.exec(
        url.pathname,
      )

    if (transitionMatch === null && dispatchMatch === null) {
      return undefined
    }

    return runRoute(
      Effect.gen(function* () {
        const authorized = yield* Effect.tryPromise({
          catch: () => new SiteReferralPayoutLedgerUnauthorized({}),
          try: () => dependencies.requireAdminApiToken(request, env),
        })

        if (!authorized) {
          return yield* new SiteReferralPayoutLedgerUnauthorized({})
        }

        if (dispatchMatch !== null) {
          if (request.method !== 'POST') {
            return methodNotAllowed(['POST'])
          }

          const parsed = yield* Effect.tryPromise({
            catch: error =>
              new SiteReferralPayoutLedgerBadRequest({
                reason: errorMessage(error),
              }),
            try: async () =>
              decodeUnknownWithSchema(
                DispatchRequest,
                await request.json().catch(() => ({})),
              ),
          })
          const payoutRef = decodeURIComponent(dispatchMatch[1] ?? '')
          const outcome = yield* Effect.tryPromise({
            catch: error =>
              error instanceof SiteReferralPayoutLedgerValidationError ||
              error instanceof SiteReferralPayoutLedgerStorageError
                ? error
                : error instanceof SiteReferralPayoutDispatchError
                  ? new SiteReferralPayoutLedgerStorageError({
                      error,
                      operation: 'siteReferralPayoutLedger.dispatchRoute.adapter',
                    })
                  : new SiteReferralPayoutLedgerStorageError({
                      error,
                      operation: 'siteReferralPayoutLedger.dispatchRoute',
                    }),
            try: () =>
              dispatchReferralPayoutSettlement(
                openAgentsDatabase(env),
                dependencies.dispatchDependencies,
                { payoutRef, revenueAsset: parsed.revenueAsset },
              ),
          })

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
                        : {
                            payoutRef: outcome.entry.payoutRef,
                            state: outcome.entry.state,
                          }),
                    },
          })
        }

        if (request.method !== 'POST') {
          return methodNotAllowed(['POST'])
        }

        const parsed = yield* Effect.tryPromise({
          catch: error =>
            new SiteReferralPayoutLedgerBadRequest({
              reason: errorMessage(error),
            }),
          try: async () =>
            decodeUnknownWithSchema(
              TransitionRequest,
              await request.json().catch(() => ({})),
            ),
        })
        const payoutRef = decodeURIComponent(transitionMatch?.[1] ?? '')
        const payout = yield* Effect.tryPromise({
          catch: error =>
            error instanceof SiteReferralPayoutLedgerValidationError ||
            error instanceof SiteReferralPayoutLedgerStorageError
              ? error
              : new SiteReferralPayoutLedgerStorageError({
                  error,
                  operation: 'siteReferralPayoutLedger.transitionRoute',
                }),
          try: () =>
            transitionReferralPayout(openAgentsDatabase(env), {
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

        return noStoreJsonResponse({
          payout: projectSiteReferralPayout(payout),
        })
      }),
    )
  },
})
