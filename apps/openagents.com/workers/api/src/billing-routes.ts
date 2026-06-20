import { Effect } from 'effect'

import {
  type BillingSummary,
  readBillingSummary,
  redeemBillingCoupon,
  upsertBillingAutoTopUpPolicy,
  withBillingCreditPackages,
} from './billing'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { fundInferenceFromCredit } from './inference/usd-credit-bridge'
import { readJsonObject } from './json-boundary'
import { firstText } from './omni-runs'
import { openAgentsDatabase } from './runtime'
import { compactRandomId } from './runtime-primitives'
import {
  type StripeBillingEnv,
  StripeCheckoutError,
  StripeConfigError,
  StripeProviderError,
  StripeWebhookError,
  makeStripeCheckoutServiceForRoutes,
  readBillingCreditPackages,
} from './stripe-billing'

type BillingEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}> &
  StripeBillingEnv

// Read the billing summary AND attach the purchasable credit catalog projected
// from the server Stripe config. Every browser-facing billing response goes
// through here so the UI always renders buy buttons whose packageId the
// `/api/billing/checkout` endpoint will accept (closes the hardcoded-id gap).
const readBillingSummaryWithPackages = async (
  environment: BillingEnv,
  userId: string,
): Promise<BillingSummary> => {
  const summary = await readBillingSummary(
    openAgentsDatabase(environment),
    userId,
  )

  return withBillingCreditPackages(
    summary,
    readBillingCreditPackages(environment),
  )
}

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

const firstNumber = (...values: ReadonlyArray<unknown>): number | undefined => {
  const number = values
    .map(value =>
      typeof value === 'number'
        ? value
        : typeof value === 'string'
          ? Number(value)
          : Number.NaN,
    )
    .find(value => Number.isFinite(value))

  return number === undefined ? undefined : Math.trunc(number)
}

const readFulfilledCheckoutSessionForUser = async (
  db: D1Database,
  input: Readonly<{ sessionId: string; userId: string }>,
): Promise<string | undefined> => {
  const row = await db
    .prepare(
      `SELECT session_id
         FROM stripe_checkout_sessions
        WHERE session_id = ?
          AND user_id = ?
          AND payment_status = 'paid'
          AND fulfillment_status = 'fulfilled'
        LIMIT 1`,
    )
    .bind(input.sessionId, input.userId)
    .first<Readonly<{ session_id: string }>>()

  if (row === null) {
    return undefined
  }

  const ledger = await db
    .prepare(
      `SELECT id
         FROM billing_ledger_entries
        WHERE source = 'stripe_checkout'
          AND amount_cents > 0
          AND idempotency_key = ?
        LIMIT 1`,
    )
    .bind(`billing:stripe-checkout:${input.sessionId}`)
    .first()

  return ledger === null ? undefined : row.session_id
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
    createSetupIntent: (input: {
      db: D1Database
      email?: string | undefined
      userId: string
    }) => Promise<
      Readonly<{ clientSecret: string; setupIntentId: string; status: string }>
    >
    saveSetupIntentPaymentMethod: (input: {
      db: D1Database
      setupIntentId: string
      userId: string
    }) => Promise<unknown>
    chargeAutoTopUp: (input: {
      db: D1Database
      idempotencyKey?: string | undefined
      userId: string
    }) => Promise<
      Readonly<{ billing: unknown; message?: string; status: string }>
    >
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
        billing: await readBillingSummaryWithPackages(
          environment,
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
            billing: await readBillingSummaryWithPackages(
              environment,
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
          // Carry the purchasable catalog so the UI keeps its buy buttons after
          // the model replaces `auth.billing` with this response's summary.
          billing: withBillingCreditPackages(
            result.billing,
            readBillingCreditPackages(environment),
          ),
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
    // The packageId MUST come from the request and MUST be a real catalog id.
    // The UI sends ids it read from the server catalog (the billing summary
    // `packages`), so there is no defensible default: an absent or unknown id
    // is a client/config mismatch and `createCreditCheckout` rejects it with a
    // clear `checkout_error` rather than silently charging the wrong package.
    const packageId = firstText(body.packageId)

    if (packageId === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(
          {
            billing: await readBillingSummaryWithPackages(
              environment,
              session.user.userId,
            ),
            error: 'package_required',
            message: 'Choose a credit package to purchase.',
          },
          { status: 400 },
        ),
        session,
      )
    }

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
          billing: await readBillingSummaryWithPackages(
            environment,
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

  // POST /api/billing/inference-credit (#5497): fund inference from the caller's
  // USD credit balance. Debits the user's USD `billing_ledger_entries` and grants
  // the equivalent msat into their agent balance (`agent:<userId>`) as a
  // USD-origin (inference-spendable, NOT Bitcoin-withdrawable) credit. Bounded by
  // the available USD balance, idempotent per client-supplied grantRef, atomic.
  handleBillingInferenceCreditApi: async (
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
    const amountCents = firstNumber(body.amountCents, body.amount_cents)
    // A client may supply a stable grantRef so a retry is idempotent (one ref =
    // one grant). Default to a per-user-stable-per-request id otherwise.
    const grantRef =
      firstText(body.grantRef, body.grant_ref) ??
      `${session.user.userId}:${compactRandomId('credit')}`
    const requestedCheckoutSessionId = firstText(
      body.sourceCheckoutSessionId,
      body.source_checkout_session_id,
    )

    if (amountCents === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(
          {
            billing: await readBillingSummaryWithPackages(
              environment,
              session.user.userId,
            ),
            error: 'amount_required',
            message: 'Enter an amount of credit (in cents) to fund inference.',
          },
          { status: 400 },
        ),
        session,
      )
    }

    const sourceCheckoutSessionId =
      requestedCheckoutSessionId === undefined
        ? undefined
        : await readFulfilledCheckoutSessionForUser(
            openAgentsDatabase(environment),
            {
              sessionId: requestedCheckoutSessionId,
              userId: session.user.userId,
            },
          )

    if (
      requestedCheckoutSessionId !== undefined &&
      sourceCheckoutSessionId === undefined
    ) {
      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(
          {
            billing: await readBillingSummaryWithPackages(
              environment,
              session.user.userId,
            ),
            error: 'checkout_session_not_fulfilled',
            message:
              'The checkout session must belong to this user and be fulfilled before it can fund inference.',
          },
          { status: 400 },
        ),
        session,
      )
    }

    const outcome = await Effect.runPromise(
      fundInferenceFromCredit(
        {
          amountCents,
          grantRef,
          ...(sourceCheckoutSessionId === undefined
            ? {}
            : { sourceCheckoutSessionId }),
          userId: session.user.userId,
        },
        { db: openAgentsDatabase(environment) },
      ),
    )

    const billing = await readBillingSummaryWithPackages(
      environment,
      session.user.userId,
    )

    if (!outcome.ok) {
      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(
          {
            billing,
            error: outcome.reason,
            message: outcome.message,
          },
          { status: 400 },
        ),
        session,
      )
    }

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({
        billing,
        grantedCents: outcome.grantedCents,
        grantedMsat: outcome.grantedMsat,
        message: 'Inference credit funded.',
        receiptRef: outcome.receiptRef,
        status: 'inference_credit_funded',
      }),
      session,
    )
  },

  handleBillingStripeSetupIntentApi: async (
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

    try {
      const stripe =
        dependencies.stripe ?? makeStripeCheckoutServiceForRoutes(environment)
      const setupIntent = await stripe.createSetupIntent({
        db: openAgentsDatabase(environment),
        email: session.user.email,
        userId: session.user.userId,
      })

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          clientSecret: setupIntent.clientSecret,
          setupIntentId: setupIntent.setupIntentId,
          status: setupIntent.status,
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

  handleBillingStripeSetupIntentSaveApi: async (
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
    const setupIntentId = firstText(body.setupIntentId, body.setup_intent)

    if (setupIntentId === undefined) {
      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(
          {
            error: 'setup_intent_required',
            message: 'SetupIntent is required.',
          },
          { status: 400 },
        ),
        session,
      )
    }

    try {
      const stripe =
        dependencies.stripe ?? makeStripeCheckoutServiceForRoutes(environment)

      await stripe.saveSetupIntentPaymentMethod({
        db: openAgentsDatabase(environment),
        setupIntentId,
        userId: session.user.userId,
      })

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          billing: await readBillingSummaryWithPackages(
            environment,
            session.user.userId,
          ),
          message: 'Card saved.',
          status: 'payment_method_saved',
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

  handleBillingAutoTopUpPolicyApi: async (
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
    const enabled = body.enabled === true || body.enabled === 'true'
    const thresholdCents = firstNumber(body.thresholdCents)
    const amountCents = firstNumber(body.amountCents)
    const monthlyCapCents = firstNumber(body.monthlyCapCents)

    if (
      thresholdCents === undefined ||
      amountCents === undefined ||
      monthlyCapCents === undefined
    ) {
      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse(
          {
            error: 'auto_top_up_policy_invalid',
            message:
              'Auto top-up threshold, amount, and monthly cap are required.',
          },
          { status: 400 },
        ),
        session,
      )
    }

    const billing = await upsertBillingAutoTopUpPolicy(
      openAgentsDatabase(environment),
      {
        amountCents,
        enabled,
        monthlyCapCents,
        thresholdCents,
        userId: session.user.userId,
      },
    )

    return dependencies.appendRefreshedSessionCookies(
      noStoreJsonResponse({
        billing: withBillingCreditPackages(
          billing,
          readBillingCreditPackages(environment),
        ),
        message: enabled ? 'Auto top-up enabled.' : 'Auto top-up disabled.',
      }),
      session,
    )
  },

  handleBillingAutoTopUpRunApi: async (
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
    const idempotencyKey = firstText(body.idempotencyKey)

    try {
      const stripe =
        dependencies.stripe ?? makeStripeCheckoutServiceForRoutes(environment)
      const result = await stripe.chargeAutoTopUp({
        db: openAgentsDatabase(environment),
        ...(idempotencyKey === undefined ? {} : { idempotencyKey }),
        userId: session.user.userId,
      })

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({
          // The service returns the post-charge summary as `unknown`; re-read it
          // authoritatively here so the response carries a typed summary WITH
          // the purchasable catalog the UI needs to keep its buy buttons.
          billing: await readBillingSummaryWithPackages(
            environment,
            session.user.userId,
          ),
          message:
            result.status === 'succeeded'
              ? 'Auto top-up completed.'
              : 'message' in result
                ? result.message
                : 'Auto top-up checked.',
          status: result.status,
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
