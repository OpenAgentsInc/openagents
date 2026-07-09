/**
 * OB-5 (#8562): CRM sales checkout-link issuance route.
 *
 *   POST /api/operator/crm/sales/checkout-link
 *     { contactId, packageId, sourceRef?, opportunityId?, tenant? }
 *     -> { link: CrmSalesCheckoutLink }
 *
 * Pack-priced only (reads the same STRIPE_CREDIT_PACKAGES_JSON catalog
 * authenticated-user billing uses) — no arbitrary-amount pricing. Admin-token
 * gated, matching the rest of the CRM operator surface
 * (crm-reply-routes.ts, crm-batch-routes.ts).
 */
import { Effect } from 'effect'

import { CrmSalesCheckoutError } from './crm-sales-checkout'
import { DEFAULT_CRM_TENANT_REF } from './crm-store'
import { isRecord } from './json-boundary'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  makeStripeCheckoutServiceForRoutes,
  type StripeBillingEnv,
} from './stripe-billing'
import type { BillingSyncEnv } from './billing-store'

type HttpResponse = globalThis.Response

type CrmSalesCheckoutEnv = StripeBillingEnv &
  BillingSyncEnv &
  Readonly<{ OPENAGENTS_DB: D1Database }>

type CrmSalesCheckoutRouteDependencies<Bindings extends CrmSalesCheckoutEnv> =
  Readonly<{
    requireAdminApiToken: (request: Request, env: Bindings) => Promise<boolean>
  }>

const CHECKOUT_LINK = /^\/api\/operator\/crm\/sales\/checkout-link$/

const ALL: ReadonlyArray<RegExp> = [CHECKOUT_LINK]

const tenantOf = (bodyTenant: unknown): string =>
  typeof bodyTenant === 'string' && bodyTenant.trim() !== ''
    ? bodyTenant.trim()
    : DEFAULT_CRM_TENANT_REF

export const makeCrmSalesCheckoutRoutes = <
  Bindings extends CrmSalesCheckoutEnv,
>(
  dependencies: CrmSalesCheckoutRouteDependencies<Bindings>,
) => {
  const guard = (
    request: Request,
    env: Bindings,
    body: () => Promise<HttpResponse>,
  ): Effect.Effect<HttpResponse> =>
    Effect.gen(function* () {
      const authorized = yield* Effect.tryPromise({
        catch: () => false as const,
        try: () => dependencies.requireAdminApiToken(request, env),
      })
      if (!authorized) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }
      return yield* Effect.tryPromise({
        catch: error =>
          error instanceof CrmSalesCheckoutError
            ? error
            : new CrmSalesCheckoutError({ reason: String(error) }),
        try: body,
      })
    }).pipe(
      Effect.catch(error =>
        Effect.succeed(
          error instanceof CrmSalesCheckoutError
            ? noStoreJsonResponse(
                { error: 'crm_sales_checkout_error', reason: error.reason },
                { status: 422 },
              )
            : noStoreJsonResponse(
                { error: 'crm_sales_checkout_error' },
                { status: 500 },
              ),
        ),
      ),
    )

  return {
    routeCrmSalesCheckoutRequest: (
      request: Request,
      env: Bindings,
      _ctx?: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)
      const path = url.pathname
      if (!ALL.some(pattern => pattern.test(path))) {
        return undefined
      }

      if (CHECKOUT_LINK.test(path)) {
        if (request.method !== 'POST') {
          return Effect.succeed(methodNotAllowed(['POST']))
        }
        return guard(request, env, async () => {
          const body = (await request.json().catch(() => null)) as Record<
            string,
            unknown
          > | null
          if (!isRecord(body)) {
            return noStoreJsonResponse(
              { error: 'bad_request', reason: 'json body required' },
              { status: 400 },
            )
          }
          const contactId =
            typeof body.contactId === 'string' ? body.contactId.trim() : ''
          const packageId =
            typeof body.packageId === 'string' ? body.packageId.trim() : ''
          if (contactId === '' || packageId === '') {
            return noStoreJsonResponse(
              {
                error: 'bad_request',
                reason: 'contactId and packageId required',
              },
              { status: 400 },
            )
          }
          const opportunityId =
            typeof body.opportunityId === 'string' &&
            body.opportunityId.trim() !== ''
              ? body.opportunityId.trim()
              : undefined
          const sourceRef =
            typeof body.sourceRef === 'string' ? body.sourceRef : undefined

          const link = await makeStripeCheckoutServiceForRoutes(
            env,
          ).createCrmSalesCheckoutLink({
            db: env.OPENAGENTS_DB,
            tenantRef: tenantOf(body.tenant),
            contactId,
            packageId,
            ...(sourceRef !== undefined ? { sourceRef } : {}),
            ...(opportunityId !== undefined ? { opportunityId } : {}),
          })
          return noStoreJsonResponse({ link }, { status: 201 })
        })
      }

      return Effect.succeed(
        noStoreJsonResponse({ error: 'not_found' }, { status: 404 }),
      )
    },
  }
}
