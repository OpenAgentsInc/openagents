import { Effect, Match as M, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { decodeUnknownWithSchema } from './json-boundary'
import { openAgentsDatabase } from './runtime'
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
    const match =
      /^\/api\/operator\/sites\/referrals\/payout-ledger\/([^/]+)\/transitions$/.exec(
        url.pathname,
      )

    if (match === null) {
      return undefined
    }

    return runRoute(
      Effect.gen(function* () {
        if (request.method !== 'POST') {
          return methodNotAllowed(['POST'])
        }

        const authorized = yield* Effect.tryPromise({
          catch: () => new SiteReferralPayoutLedgerUnauthorized({}),
          try: () => dependencies.requireAdminApiToken(request, env),
        })

        if (!authorized) {
          return yield* new SiteReferralPayoutLedgerUnauthorized({})
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
        const payoutRef = decodeURIComponent(match[1] ?? '')
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
