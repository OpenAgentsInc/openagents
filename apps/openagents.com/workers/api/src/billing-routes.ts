import { readBillingSummary, redeemBillingCoupon } from './billing'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { readJsonObject } from './json-boundary'
import { firstText } from './omni-runs'
import { openAgentsDatabase } from './runtime'
import {
  type StripeBillingEnv,
  StripeCheckoutError,
  StripeConfigError,
  StripeProviderError,
  StripeWebhookError,
  makeStripeCheckoutServiceForRoutes,
} from './stripe-billing'

type BillingEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}> &
  StripeBillingEnv

type BillingSession = Readonly<{
  user: Readonly<{
    email?: string | undefined
    userId: string
  }>
}>

const billingRouteErrorResponse = (error: unknown): Response => {
  if (error instanceof StripeConfigError) {
    return noStoreJsonResponse(
      {
        error: 'stripe_unconfigured',
        message: 'Card checkout is not available yet.',
      },
      { status: 503 },
    )
  }

  if (error instanceof StripeCheckoutError) {
    return noStoreJsonResponse(
      { error: 'checkout_error', message: error.reason },
      { status: 400 },
    )
  }

  if (error instanceof StripeWebhookError) {
    return noStoreJsonResponse(
      { error: 'webhook_error', message: error.reason },
      { status: 400 },
    )
  }

  if (error instanceof StripeProviderError) {
    return noStoreJsonResponse(
      {
        error: 'stripe_provider_error',
        message: 'Stripe request failed.',
        operation: error.operation,
      },
      { status: 502 },
    )
  }

  return noStoreJsonResponse(
    { error: 'billing_error', message: 'Billing request failed.' },
    { status: 500 },
  )
}

type BillingApiDependencies<
  Session extends BillingSession,
  Env extends BillingEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: Response,
    session: Session,
  ) => Response
  requireBrowserSession: (
    request: Request,
    environment: Env,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
  stripe?: Readonly<{
    createCreditCheckout: (input: {
      db: D1Database
      email?: string | undefined
      packageId: string
      userId: string
    }) => Promise<Readonly<{ checkoutUrl: string }>>
    fulfillCheckoutSession: (input: {
      db: D1Database
      sessionId: string
    }) => Promise<unknown>
    processWebhook: (input: {
      db: D1Database
      payload: string
      signature: string | null
    }) => Promise<unknown>
  }>
}>

export const makeBillingApiHandlers = <
  Session extends BillingSession,
  Env extends BillingEnv,
>(
  dependencies: BillingApiDependencies<Session, Env>,
) => ({
  handleBillingSummaryApi: async (
    request: Request,
    environment: Env,
    ctx: ExecutionContext,
  ) => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const session = await dependencies.requireBrowserSession(
      request,
      environment,
      ctx,
    )

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({
        billing: await readBillingSummary(
          openAgentsDatabase(environment),
          session.user.userId,
        ),
      }),
      session,
    )
  },

  handleBillingCouponRedeemApi: async (
    request: Request,
    environment: Env,
    ctx: ExecutionContext,
  ) => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = await dependencies.requireBrowserSession(
      request,
      environment,
      ctx,
    )

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const couponCode = firstText(body.couponCode, body.code)

    if (couponCode === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(
          {
            billing: await readBillingSummary(
              openAgentsDatabase(environment),
              session.user.userId,
            ),
            error: 'coupon_code_required',
            message: 'Enter a coupon code.',
          },
          { status: 400 },
        ),
        session,
      )
    }

    const result = await redeemBillingCoupon(openAgentsDatabase(environment), {
      couponCode,
      userId: session.user.userId,
    })

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse(
        {
          billing: result.billing,
          ...(result.ok ? {} : { error: result.error }),
          message: result.message,
        },
        { status: result.ok ? 200 : 400 },
      ),
      session,
    )
  },

  handleBillingCheckoutApi: async (
    request: Request,
    environment: Env,
    ctx: ExecutionContext,
  ) => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const session = await dependencies.requireBrowserSession(
      request,
      environment,
      ctx,
    )

    if (session === undefined) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const body = await readJsonObject(request).catch(
      (): Record<string, unknown> => ({}),
    )
    const packageId = firstText(body.packageId) ?? 'starter'

    try {
      const stripe =
        dependencies.stripe ?? makeStripeCheckoutServiceForRoutes(environment)
      const checkout = await stripe.createCreditCheckout({
        db: openAgentsDatabase(environment),
        email: session.user.email,
        packageId,
        userId: session.user.userId,
      })

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          billing: await readBillingSummary(
            openAgentsDatabase(environment),
            session.user.userId,
          ),
          checkoutUrl: checkout.checkoutUrl,
          message: 'Opening secure checkout.',
          packageId,
          status: 'checkout_created',
        }),
        session,
      )
    } catch (error) {
      return dependencies.appendRefreshedSessionCookies(
        billingRouteErrorResponse(error),
        session,
      )
    }
  },

  handleBillingStripeWebhookApi: async (request: Request, environment: Env) => {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    try {
      const stripe =
        dependencies.stripe ?? makeStripeCheckoutServiceForRoutes(environment)
      const result = await stripe.processWebhook({
        db: openAgentsDatabase(environment),
        payload: await request.text(),
        signature: request.headers.get('Stripe-Signature'),
      })

      return noStoreJsonResponse({ result })
    } catch (error) {
      return billingRouteErrorResponse(error)
    }
  },

  handleBillingStripeCheckoutReturnApi: async (
    request: Request,
    environment: Env,
  ) => {
    if (request.method !== 'GET') {
      return methodNotAllowed(['GET'])
    }

    const url = new URL(request.url)
    const sessionId = url.searchParams.get('session_id')
    const redirect = new URL('/billing', url.origin)

    if (sessionId !== null && sessionId.trim() !== '') {
      await Promise.resolve()
        .then(() => {
          const stripe =
            dependencies.stripe ??
            makeStripeCheckoutServiceForRoutes(environment)

          return stripe.fulfillCheckoutSession({
            db: openAgentsDatabase(environment),
            sessionId,
          })
        })
        .catch(() => undefined)
    }

    return new Response(null, {
      headers: { location: redirect.toString() },
      status: 303,
    })
  },
})
