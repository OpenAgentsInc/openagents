/**
 * Operator route module for seeding/reading partner agreements (#5524 follow-up).
 *
 * Advances `blocker.product_promises.partner_attribution_policy_missing` by
 * supplying the one remaining CALL-SITE the storage layer already had functions
 * for but no HTTP surface to reach:
 *
 *   recordPartnerAgreement()                  -- the sanctioned, policy-validated
 *     (partner-payout-feed.ts)                   WRITER of one `partner_agreements`
 *                                                row (migration 0214).
 *   readActivePartnerAgreementsForCustomer()  -- the bounded, read-only reader the
 *     (partner-payout-feed.ts)                   attribution policy depends on.
 *
 * Before this module the only way to create the EXPLICIT partner agreements the
 * attribution policy reads from was a raw SQL insert. The writer enforces the
 * attribution invariants at the write boundary (referral-role exclusion,
 * self-agreement exclusion, window consistency, public-safe refs), but nothing
 * could call it over HTTP, so operators had no sanctioned path to seed an
 * agreement — and without an agreement, no partner is ever attributed (the
 * no-fallback rule that distinguishes this rail from the inferred referral feed).
 *
 * This is the partner-agreement analogue of `partner-payout-ledger-routes.ts`:
 * a thin, admin-gated surface over existing pure/storage functions. It NEVER
 * moves money; it only records WHO MAY be attributed. Settlement stays
 * operator-gated through the payout ledger's approve/dispatch/settle states.
 *
 * Endpoints:
 *   POST /api/operator/partners/agreements
 *     - admin-gated; decodes the agreement seed and calls
 *       `recordPartnerAgreement`. Idempotent on `agreementRef` (replays return
 *       the stored agreement unchanged). A policy-violating seed is a 422.
 *   GET  /api/operator/partners/agreements?customerUserId=<id>
 *     - admin-gated; returns the currently-active agreements covering the
 *       customer (read-back verification for a just-seeded row).
 *
 * Refs/ids here are admin-only operator fields; this is NOT the public-safe
 * projection surface (`GET /api/public/partner-payouts`), which exposes only
 * count-only aggregates.
 *
 * ============================================================================
 * COORDINATOR WIRING (deferred integration — do NOT wire from this lane)
 * ----------------------------------------------------------------------------
 * Mirrors the `partner-payout-ledger-routes.ts` wiring. The coordinator must:
 *
 *   import { makePartnerAgreementRoutes } from './partner-agreement-routes'
 *
 *   const partnerAgreementRoutes = makePartnerAgreementRoutes({
 *     nowIso: currentIsoTimestamp,
 *     requireAdminApiToken,
 *   })
 *
 * then expose `partnerAgreementRoutes.routePartnerAgreementRequest` on the
 * worker-routes dependency object and chain it in `routeOmniRequest` next to the
 * partner payout ledger block. Apply migration `0214_partner_agreements.sql`
 * before serving. Path prefixes do not collide with the payout-ledger routes.
 * ============================================================================
 */
import { Effect, Match as M, Schema as S } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { decodeUnknownWithSchema } from './json-boundary'
import { type PartnerAgreement } from './partner-attribution-policy'
import {
  PartnerAgreementValidationError,
  readActivePartnerAgreementsForCustomer,
  recordPartnerAgreement,
} from './partner-payout-feed'
import {
  PartnerPayoutLedgerStorageError,
  PartnerPayoutRole,
} from './partner-payout-ledger'
import { openAgentsDatabase } from './runtime'
import { makeTreasuryDatabaseForEnv } from './treasury-domain-store'

type HttpResponse = globalThis.Response

type PartnerAgreementEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

type PartnerAgreementRouteDependencies<Bindings extends PartnerAgreementEnv> =
  Readonly<{
    nowIso: () => string
    requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  }>

class PartnerAgreementUnauthorized extends S.TaggedErrorClass<PartnerAgreementUnauthorized>()(
  'PartnerAgreementUnauthorized',
  {},
) {}

class PartnerAgreementBadRequest extends S.TaggedErrorClass<PartnerAgreementBadRequest>()(
  'PartnerAgreementBadRequest',
  {
    reason: S.String,
  },
) {}

type PartnerAgreementRouteError =
  | PartnerAgreementBadRequest
  | PartnerAgreementUnauthorized
  | PartnerAgreementValidationError
  | PartnerPayoutLedgerStorageError

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

/**
 * The agreement seed an operator may POST. `role` deliberately accepts the full
 * `PartnerPayoutRole` set so a `referral` seed yields the writer's informative
 * 422 ("the referral rail owns referral payouts") rather than a generic schema
 * error; the writer is the single source of truth for attribution invariants.
 */
const CreateAgreementRequest = S.Struct({
  agreementRef: S.Trim.check(S.isMinLength(1), S.isMaxLength(220)),
  customerUserId: S.Trim.check(S.isMinLength(1), S.isMaxLength(220)),
  effectiveFromIso: S.Trim.check(S.isMinLength(1), S.isMaxLength(60)),
  effectiveUntilIso: S.optionalKey(S.NullOr(S.Trim.check(S.isMaxLength(60)))),
  id: S.optionalKey(S.Trim.check(S.isMinLength(1), S.isMaxLength(220))),
  partnerRef: S.Trim.check(S.isMinLength(1), S.isMaxLength(220)),
  partnerUserId: S.Trim.check(S.isMinLength(1), S.isMaxLength(220)),
  role: PartnerPayoutRole,
})

/**
 * Operator-facing projection of a stored agreement. `PartnerAgreement` already
 * omits `customerUserId`; this echoes the persisted shape for read-back.
 */
const projectAgreement = (agreement: PartnerAgreement) => ({
  agreementRef: agreement.agreementRef,
  effectiveFromIso: agreement.effectiveFromIso,
  effectiveUntilIso: agreement.effectiveUntilIso,
  partnerRef: agreement.partnerRef,
  partnerRole: agreement.role,
  partnerUserId: agreement.partnerUserId,
})

const routeErrorResponse = (
  error: PartnerAgreementRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      PartnerAgreementBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      PartnerAgreementValidationError: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'partner_agreement_rejected', reason },
          { status: 422 },
        ),
      PartnerPayoutLedgerStorageError: () =>
        noStoreJsonResponse(
          { error: 'partner_agreement_storage_error' },
          { status: 500 },
        ),
      PartnerAgreementUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
    }),
    M.exhaustive,
  )

const runRoute = (
  effect: Effect.Effect<HttpResponse, PartnerAgreementRouteError>,
): Effect.Effect<HttpResponse> =>
  effect.pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

const requireAdmin = <Bindings extends PartnerAgreementEnv>(
  dependencies: PartnerAgreementRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<void, PartnerAgreementRouteError> =>
  Effect.gen(function* () {
    const authorized = yield* Effect.tryPromise({
      catch: () => new PartnerAgreementUnauthorized({}),
      try: () => dependencies.requireAdminApiToken(request, env),
    })

    if (!authorized) {
      return yield* new PartnerAgreementUnauthorized({})
    }
  })

const createRoute = <Bindings extends PartnerAgreementEnv>(
  dependencies: PartnerAgreementRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
): Effect.Effect<HttpResponse> =>
  runRoute(
    Effect.gen(function* () {
      yield* requireAdmin(dependencies, request, env)

      const parsed = yield* Effect.tryPromise({
        catch: error =>
          new PartnerAgreementBadRequest({ reason: errorMessage(error) }),
        try: async () =>
          decodeUnknownWithSchema(
            CreateAgreementRequest,
            await request.json().catch(() => ({})),
          ),
      })

      const agreement = yield* Effect.tryPromise({
        catch: error =>
          error instanceof PartnerAgreementValidationError ||
          error instanceof PartnerPayoutLedgerStorageError
            ? error
            : new PartnerPayoutLedgerStorageError({
                error,
                operation: 'partnerAgreement.createRoute',
              }),
        try: () =>
          recordPartnerAgreement(makeTreasuryDatabaseForEnv(env), {
            agreementRef: parsed.agreementRef,
            customerUserId: parsed.customerUserId,
            effectiveFromIso: parsed.effectiveFromIso,
            nowIso: dependencies.nowIso(),
            partnerRef: parsed.partnerRef,
            partnerUserId: parsed.partnerUserId,
            role: parsed.role,
            ...(parsed.effectiveUntilIso === undefined
              ? {}
              : { effectiveUntilIso: parsed.effectiveUntilIso }),
            ...(parsed.id === undefined ? {} : { id: parsed.id }),
          }),
      })

      return noStoreJsonResponse({ agreement: projectAgreement(agreement) })
    }),
  )

const listRoute = <Bindings extends PartnerAgreementEnv>(
  dependencies: PartnerAgreementRouteDependencies<Bindings>,
  request: Request,
  env: Bindings,
  url: URL,
): Effect.Effect<HttpResponse> =>
  runRoute(
    Effect.gen(function* () {
      yield* requireAdmin(dependencies, request, env)

      const customerUserId = url.searchParams.get('customerUserId')

      if (customerUserId === null || customerUserId.trim() === '') {
        return yield* new PartnerAgreementBadRequest({
          reason: 'customerUserId query parameter is required.',
        })
      }

      const agreements = yield* Effect.tryPromise({
        catch: error =>
          new PartnerPayoutLedgerStorageError({
            error,
            operation: 'partnerAgreement.listRoute',
          }),
        try: () =>
          readActivePartnerAgreementsForCustomer(
            openAgentsDatabase(env),
            customerUserId,
          ),
      })

      return noStoreJsonResponse({
        agreements: agreements.map(projectAgreement),
      })
    }),
  )

const COLLECTION_PATTERN = /^\/api\/operator\/partners\/agreements$/

export const makePartnerAgreementRoutes = <
  Bindings extends PartnerAgreementEnv,
>(
  dependencies: PartnerAgreementRouteDependencies<Bindings>,
) => ({
  routePartnerAgreementRequest: (
    request: Request,
    env: Bindings,
    _ctx?: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (COLLECTION_PATTERN.exec(url.pathname) === null) {
      return undefined
    }

    if (request.method === 'POST') {
      return createRoute(dependencies, request, env)
    }

    if (request.method === 'GET') {
      return listRoute(dependencies, request, env, url)
    }

    return Effect.succeed(methodNotAllowed(['GET', 'POST']))
  },
})
