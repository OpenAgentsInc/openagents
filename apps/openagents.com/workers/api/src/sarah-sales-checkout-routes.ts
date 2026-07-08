import { Effect, Match as M } from 'effect'

import { methodNotAllowed, noStoreJsonResponse, unauthorized } from './http/responses'
import { optionalInteger, optionalString, readJsonObject, stringArrayFromUnknown } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  type StripeBillingEnv,
  StripeCheckoutError,
  StripeConfigError,
  StripeProviderError,
  makeStripeCheckoutServiceForRoutes,
  readBillingCreditPackages,
} from './stripe-billing'
import type { BillingSyncEnv } from './billing-store'

type SarahCheckoutEnv = StripeBillingEnv &
  BillingSyncEnv &
  Readonly<{
    OPENAGENTS_APP_URL?: string | undefined
    OPENAGENTS_DB: D1Database
    SARAH_SALES_CHECKOUT_TEST_MODE?: string | undefined
  }>

type CreditPackage = Readonly<{
  id: string
  paidAmountCents: number
}>

type SarahCheckoutInput = Readonly<{
  amountUsdCents: number
  buyerUserId: string | null
  contactEmail: string
  dealRuleRefs: ReadonlyArray<string>
  packageId: string
  quoteRef: string
  signupId: string | null
  sourceRef: string
}>

type CreateCreditCheckout = (input: {
  businessKickoff?: Readonly<{ signupId: string }> | undefined
  db: D1Database
  email?: string | undefined
  packageId: string
  userId: string
}) => Promise<Readonly<{ checkoutUrl: string; sessionId?: string | undefined }>>

type SarahSalesCheckoutDependencies<Env extends SarahCheckoutEnv> = Readonly<{
  createCreditCheckout?: CreateCreditCheckout
  makeDb: (env: Env) => D1Database
  nowIso?: () => string
  readCreditPackages?: (env: Env) => ReadonlyArray<CreditPackage>
  requireAdminApiToken: (request: Request, env: Env) => Promise<boolean>
  testModeEnabled?: (env: Env) => boolean
}>

class SarahSalesCheckoutRouteError extends Error {
  constructor(
    readonly kind:
      | 'checkout_error'
      | 'provider_error'
      | 'stripe_unconfigured'
      | 'validation_error',
    readonly reason: string,
  ) {
    super(reason)
  }
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const emailPattern = /^[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+$/

const trimText = (value: string | undefined): string | undefined => {
  const trimmed = value?.trim()

  return trimmed === undefined || trimmed === '' ? undefined : trimmed
}

const requiredText = (
  body: Record<string, unknown>,
  field: string,
  maxLength: number,
): string => {
  const value = trimText(optionalString(body[field]))?.slice(0, maxLength)

  if (value === undefined) {
    throw new SarahSalesCheckoutRouteError(
      'validation_error',
      `${field} is required.`,
    )
  }

  return value
}

const safeOptionalRef = (
  body: Record<string, unknown>,
  field: string,
): string | null => {
  const value = trimText(optionalString(body[field]))

  if (value === undefined) {
    return null
  }

  if (!safeRefPattern.test(value)) {
    throw new SarahSalesCheckoutRouteError(
      'validation_error',
      `${field} must be an opaque public-safe ref.`,
    )
  }

  return value
}

const safeRequiredRef = (
  body: Record<string, unknown>,
  field: string,
): string => {
  const value = requiredText(body, field, 220)

  if (!safeRefPattern.test(value)) {
    throw new SarahSalesCheckoutRouteError(
      'validation_error',
      `${field} must be an opaque public-safe ref.`,
    )
  }

  return value
}

const inputFromBody = (body: Record<string, unknown>): SarahCheckoutInput => {
  const amountUsdCents =
    optionalInteger(body.amountUsdCents) ?? optionalInteger(body.amount_usd_cents)

  if (
    amountUsdCents === undefined ||
    amountUsdCents < 100_000 ||
    amountUsdCents > 1_000_000
  ) {
    throw new SarahSalesCheckoutRouteError(
      'validation_error',
      'amountUsdCents must be an integer between 100000 and 1000000.',
    )
  }

  const contactEmail = requiredText(body, 'contactEmail', 320).toLowerCase()

  if (!emailPattern.test(contactEmail)) {
    throw new SarahSalesCheckoutRouteError(
      'validation_error',
      'contactEmail must be a valid email address.',
    )
  }

  const dealRuleRefs = stringArrayFromUnknown(body.dealRuleRefs)
    .map(ref => ref.trim())
    .filter(ref => ref !== '')

  if (dealRuleRefs.length === 0) {
    throw new SarahSalesCheckoutRouteError(
      'validation_error',
      'dealRuleRefs must contain at least one configured deal-rule ref.',
    )
  }

  return {
    amountUsdCents,
    buyerUserId: safeOptionalRef(body, 'buyerUserId'),
    contactEmail,
    dealRuleRefs,
    packageId: safeRequiredRef(body, 'packageId'),
    quoteRef: safeRequiredRef(body, 'quoteRef'),
    signupId: safeOptionalRef(body, 'signupId'),
    sourceRef: safeRequiredRef(body, 'sourceRef'),
  }
}

const routeErrorResponse = (error: unknown): Response => {
  if (error instanceof StripeConfigError) {
    return noStoreJsonResponse(
      {
        error: 'sarah_checkout_stripe_unconfigured',
        reason: 'Card checkout is not configured for Sarah sales.',
      },
      { status: 503 },
    )
  }

  if (error instanceof StripeCheckoutError) {
    return noStoreJsonResponse(
      { error: 'sarah_checkout_error', reason: error.reason },
      { status: 400 },
    )
  }

  if (error instanceof StripeProviderError) {
    return noStoreJsonResponse(
      {
        error: 'sarah_checkout_provider_error',
        operation: error.operation,
        reason: 'Stripe request failed.',
      },
      { status: 502 },
    )
  }

  if (error instanceof SarahSalesCheckoutRouteError) {
    return M.value(error.kind).pipe(
      M.when('stripe_unconfigured', () =>
        noStoreJsonResponse(
          { error: 'sarah_checkout_stripe_unconfigured', reason: error.reason },
          { status: 503 },
        ),
      ),
      M.when('provider_error', () =>
        noStoreJsonResponse(
          { error: 'sarah_checkout_provider_error', reason: error.reason },
          { status: 502 },
        ),
      ),
      M.when('checkout_error', () =>
        noStoreJsonResponse(
          { error: 'sarah_checkout_error', reason: error.reason },
          { status: 400 },
        ),
      ),
      M.orElse(() =>
        noStoreJsonResponse(
          { error: 'sarah_checkout_validation_error', reason: error.reason },
          { status: 400 },
        ),
      ),
    )
  }

  return noStoreJsonResponse(
    {
      error: 'sarah_checkout_failed',
      reason: 'Sarah checkout link creation failed.',
    },
    { status: 500 },
  )
}

const routeCreateError = (error: unknown) => {
  if (
    error instanceof SarahSalesCheckoutRouteError ||
    error instanceof StripeConfigError ||
    error instanceof StripeCheckoutError ||
    error instanceof StripeProviderError
  ) {
    return error
  }

  return new SarahSalesCheckoutRouteError(
    'checkout_error',
    error instanceof Error ? error.message : String(error),
  )
}

const appOrigin = (env: SarahCheckoutEnv, request: Request): string => {
  const configured = trimText(env.OPENAGENTS_APP_URL)

  return configured === undefined ? new URL(request.url).origin : configured
}

const resolvePackage = (
  packages: ReadonlyArray<CreditPackage>,
  input: SarahCheckoutInput,
): CreditPackage => {
  const pack = packages.find(candidate => candidate.id === input.packageId)

  if (pack === undefined) {
    throw new SarahSalesCheckoutRouteError(
      'validation_error',
      'packageId is not in the OpenAgents checkout catalog.',
    )
  }

  if (pack.paidAmountCents !== input.amountUsdCents) {
    throw new SarahSalesCheckoutRouteError(
      'validation_error',
      'amountUsdCents must match the OpenAgents checkout catalog package.',
    )
  }

  return pack
}

const testModeResponse = (
  request: Request,
  env: SarahCheckoutEnv,
  input: SarahCheckoutInput,
  nowIso: string,
): Response => {
  const checkoutRef = compactRandomId('sarah_checkout_test')
  const receiptRef = `receipt.operator.sarah_checkout_test.${checkoutRef}`
  const checkoutUrl = `${appOrigin(env, request)}/business?sarah_checkout_ref=${encodeURIComponent(checkoutRef)}`

  return noStoreJsonResponse(
    {
      amountUsdCents: input.amountUsdCents,
      checkoutRef,
      checkoutUrl,
      dealRuleRefs: input.dealRuleRefs,
      generatedAt: nowIso,
      mode: 'test',
      moneyMovement: 'none',
      packageId: input.packageId,
      quoteRef: input.quoteRef,
      receiptRef,
      sourceRef: input.sourceRef,
      status: 'checkout_test_mode',
    },
    { status: 201 },
  )
}

const routeCreate = <Env extends SarahCheckoutEnv>(
  dependencies: SarahSalesCheckoutDependencies<Env>,
  request: Request,
  env: Env,
) =>
  Effect.tryPromise({
    catch: routeCreateError,
    try: async () => {
      if (!(await dependencies.requireAdminApiToken(request, env))) {
        return unauthorized()
      }

      const body = await readJsonObject(request)
      const input = inputFromBody(body)
      const packages =
        dependencies.readCreditPackages?.(env) ?? readBillingCreditPackages(env)
      resolvePackage(packages, input)

      const nowIso = dependencies.nowIso?.() ?? currentIsoTimestamp()
      const testMode =
        dependencies.testModeEnabled?.(env) ??
        env.SARAH_SALES_CHECKOUT_TEST_MODE === '1'

      if (testMode) {
        return testModeResponse(request, env, input, nowIso)
      }

      if (input.buyerUserId === null) {
        throw new SarahSalesCheckoutRouteError(
          'validation_error',
          'buyerUserId is required for live Stripe checkout.',
        )
      }

      const checkout: CreateCreditCheckout =
        dependencies.createCreditCheckout ??
        (async checkoutInput => {
          const snapshot = await makeStripeCheckoutServiceForRoutes(
            env,
          ).createCreditCheckout(
            checkoutInput,
          )

          if (snapshot.checkoutUrl === undefined) {
            throw new SarahSalesCheckoutRouteError(
              'checkout_error',
              'Stripe did not return a Checkout URL.',
            )
          }

          return {
            checkoutUrl: snapshot.checkoutUrl,
            sessionId: snapshot.sessionId,
          }
        })
      const created = await checkout({
        ...(input.signupId === null
          ? {}
          : { businessKickoff: { signupId: input.signupId } }),
        db: dependencies.makeDb(env),
        email: input.contactEmail,
        packageId: input.packageId,
        userId: input.buyerUserId,
      })
      const checkoutRef = compactRandomId('sarah_checkout')

      return noStoreJsonResponse(
        {
          amountUsdCents: input.amountUsdCents,
          checkoutRef,
          checkoutUrl: created.checkoutUrl,
          dealRuleRefs: input.dealRuleRefs,
          generatedAt: nowIso,
          mode: 'live',
          packageId: input.packageId,
          quoteRef: input.quoteRef,
          sourceRef: input.sourceRef,
          status: 'checkout_created',
        },
        { status: 201 },
      )
    },
  }).pipe(Effect.catch(error => Effect.succeed(routeErrorResponse(error))))

export const makeOperatorSarahSalesCheckoutRoutes = <Env extends SarahCheckoutEnv>(
  dependencies: SarahSalesCheckoutDependencies<Env>,
) => ({
  routeOperatorSarahSalesCheckoutRequest: (
    request: Request,
    env: Env,
    _ctx: ExecutionContext,
  ): Effect.Effect<Response> | undefined => {
    const url = new URL(request.url)

    if (url.pathname !== '/api/operator/business/sarah-checkout-links') {
      return undefined
    }

    if (request.method !== 'POST') {
      return Effect.succeed(methodNotAllowed(['POST']))
    }

    return routeCreate(dependencies, request, env)
  },
})
