import { Effect, Layer, Redacted, Schema as S } from 'effect'
import * as Context from 'effect/Context'
import Stripe from 'stripe'

import {
  BILLING_CURRENCY,
  type BillingCreditPackageDisplay,
  type BillingSummary,
  applyStripeAutoTopUpCredit,
  applyStripeCheckoutCredit,
  formatUsdCents,
  pauseBillingAutoTopUpPolicy,
  readBillingBalanceCents,
  readBillingSummary,
  recordBillingAutoTopUpEvent,
  systemBillingRuntime,
} from './billing'
import { WorkerSecret, redactedValue } from './config'
import { parseJsonWithSchema } from './json-boundary'
import { type PartnerQualifyingPaidEvent } from './partner-attribution-eligibility'
import { recordPartnerPayoutForPaidEvent } from './partner-payout-feed'
import { provisionBusinessCheckoutKickoff } from './business-checkout-kickoff'
import { recordReferralPayoutForPaidEvent } from './site-referral-payout-feed'

export const STRIPE_API_VERSION = '2026-05-27.dahlia'

export const StripeCustomerId = S.String.pipe(S.brand('StripeCustomerId'))
export type StripeCustomerId = typeof StripeCustomerId.Type
export const StripeCheckoutSessionId = S.String.pipe(
  S.brand('StripeCheckoutSessionId'),
)
export type StripeCheckoutSessionId = typeof StripeCheckoutSessionId.Type
export const StripeEventId = S.String.pipe(S.brand('StripeEventId'))
export type StripeEventId = typeof StripeEventId.Type
export const StripePriceId = S.String.pipe(S.brand('StripePriceId'))
export type StripePriceId = typeof StripePriceId.Type
export const StripeSetupIntentId = S.String.pipe(S.brand('StripeSetupIntentId'))
export type StripeSetupIntentId = typeof StripeSetupIntentId.Type
export const StripePaymentIntentId = S.String.pipe(
  S.brand('StripePaymentIntentId'),
)
export type StripePaymentIntentId = typeof StripePaymentIntentId.Type

export const BillingCreditPackageId = S.String.pipe(
  S.brand('BillingCreditPackageId'),
)
export type BillingCreditPackageId = typeof BillingCreditPackageId.Type

export type StripeCreditPackage = Readonly<{
  amountCents: number
  bonusCents: number
  totalCreditCents: number
  currency: 'USD'
  creditsExpire: false
  id: BillingCreditPackageId
  label: string
  priceId: StripePriceId
}>

export type StripeCheckoutSnapshot = Readonly<{
  amountCents: number
  checkoutUrl?: string | undefined
  currency: 'USD'
  customerId: StripeCustomerId
  packageId: BillingCreditPackageId
  paymentStatus: string
  sessionId: StripeCheckoutSessionId
  userId: string
}>

export type StripeBusinessCheckoutKickoffInput = Readonly<{
  signupId: string
}>

export type StripeWebhookResult = Readonly<{
  eventId: StripeEventId
  status: 'processed' | 'ignored'
  type: string
}>

export type StripeSetupIntentSnapshot = Readonly<{
  clientSecret: string
  customerId: StripeCustomerId
  setupIntentId: StripeSetupIntentId
  status: string
}>

export type StripeSavedPaymentMethodSnapshot = Readonly<{
  brand: string | null
  expMonth: number | null
  expYear: number | null
  last4: string | null
  paymentMethodId: string
  setupIntentId: StripeSetupIntentId
  status: string
}>

export type StripeAutoTopUpChargeResult =
  | Readonly<{ billing: BillingSummary; status: 'succeeded' }>
  | Readonly<{
      billing: BillingSummary
      message: string
      status: 'cap_reached' | 'declined' | 'requires_payment_method' | 'skipped'
    }>

export class StripeConfigError extends S.TaggedErrorClass<StripeConfigError>()(
  'StripeConfigError',
  {
    field: S.String,
    reason: S.String,
  },
) {}

export class StripeProviderError extends S.TaggedErrorClass<StripeProviderError>()(
  'StripeProviderError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class StripeCheckoutError extends S.TaggedErrorClass<StripeCheckoutError>()(
  'StripeCheckoutError',
  {
    reason: S.String,
  },
) {}

export class StripeWebhookError extends S.TaggedErrorClass<StripeWebhookError>()(
  'StripeWebhookError',
  {
    reason: S.String,
  },
) {}

export type StripeBillingEnv = Readonly<{
  OPENAGENTS_APP_URL?: string | undefined
  STRIPE_API_KEY?: string | undefined
  STRIPE_CHECKOUT_CANCEL_URL?: string | undefined
  STRIPE_CHECKOUT_SUCCESS_URL?: string | undefined
  STRIPE_CREDIT_PACKAGES_JSON?: string | undefined
  STRIPE_WEBHOOK_SIGNING_SECRET?: string | undefined
}>

export type StripeConfigShape = Readonly<{
  apiKey: Redacted.Redacted<WorkerSecret>
  apiVersion: typeof STRIPE_API_VERSION
  cancelUrl: string
  packages: ReadonlyMap<BillingCreditPackageId, StripeCreditPackage>
  successUrl: string
  webhookSigningSecret: Redacted.Redacted<WorkerSecret>
}>

export class StripeConfig extends Context.Service<
  StripeConfig,
  StripeConfigShape
>()('@openagentsinc/StripeConfig') {
  static layer = (env: StripeBillingEnv) =>
    Layer.effect(StripeConfig, decodeStripeConfig(env))
}

export type StripeClientShape = Readonly<{
  client: Effect.Effect<Stripe, StripeProviderError>
  unsafeClient: () => Stripe
}>

export class StripeClient extends Context.Service<
  StripeClient,
  StripeClientShape
>()('@openagentsinc/StripeClient') {
  static Live = Layer.effect(
    StripeClient,
    Effect.gen(function* () {
      const config = yield* StripeConfig
      let maybeClient: Stripe | undefined

      return {
        client: Effect.try({
          catch: error => stripeProviderError('construct_client', error),
          try: () => {
            if (maybeClient === undefined) {
              maybeClient = new Stripe(redactedValue(config.apiKey) ?? '', {
                apiVersion: config.apiVersion,
                httpClient: Stripe.createFetchHttpClient(),
                maxNetworkRetries: 2,
                timeout: 20_000,
              })
            }

            return maybeClient
          },
        }),
        unsafeClient: () => {
          if (maybeClient === undefined) {
            maybeClient = new Stripe(redactedValue(config.apiKey) ?? '', {
              apiVersion: config.apiVersion,
              httpClient: Stripe.createFetchHttpClient(),
              maxNetworkRetries: 2,
              timeout: 20_000,
            })
          }

          return maybeClient
        },
      }
    }),
  )
}

export type StripeCustomerServiceShape = Readonly<{
  ensureCustomer: (input: {
    db: D1Database
    email?: string | undefined
    userId: string
  }) => Effect.Effect<StripeCustomerId, StripeProviderError>
}>

export class StripeCustomerService extends Context.Service<
  StripeCustomerService,
  StripeCustomerServiceShape
>()('@openagentsinc/StripeCustomerService') {}

export const StripeCustomerServiceLive = Layer.effect(
  StripeCustomerService,
  Effect.gen(function* () {
    const stripeClient = yield* StripeClient

    return {
      ensureCustomer: input =>
        Effect.tryPromise({
          catch: error => stripeProviderError('ensure_customer', error),
          try: () => ensureStripeCustomer(stripeClient, input),
        }),
    }
  }),
)

export type StripeCheckoutServiceShape = Readonly<{
  createCreditCheckout: (input: {
    businessKickoff?: StripeBusinessCheckoutKickoffInput | undefined
    db: D1Database
    email?: string | undefined
    packageId: string
    userId: string
  }) => Effect.Effect<
    StripeCheckoutSnapshot,
    StripeCheckoutError | StripeProviderError
  >
  fulfillCheckoutSession: (input: {
    db: D1Database
    sessionId: string
  }) => Effect.Effect<BillingSummary, StripeCheckoutError | StripeProviderError>
  createSetupIntent: (input: {
    db: D1Database
    email?: string | undefined
    userId: string
  }) => Effect.Effect<
    StripeSetupIntentSnapshot,
    StripeCheckoutError | StripeProviderError
  >
  saveSetupIntentPaymentMethod: (input: {
    db: D1Database
    setupIntentId: string
    userId: string
  }) => Effect.Effect<
    StripeSavedPaymentMethodSnapshot,
    StripeCheckoutError | StripeProviderError
  >
  chargeAutoTopUp: (input: {
    db: D1Database
    idempotencyKey?: string | undefined
    userId: string
  }) => Effect.Effect<
    StripeAutoTopUpChargeResult,
    StripeCheckoutError | StripeProviderError
  >
}>

export class StripeCheckoutService extends Context.Service<
  StripeCheckoutService,
  StripeCheckoutServiceShape
>()('@openagentsinc/StripeCheckoutService') {}

export const StripeCheckoutServiceLive = Layer.effect(
  StripeCheckoutService,
  Effect.gen(function* () {
    const config = yield* StripeConfig
    const stripeClient = yield* StripeClient

    return {
      createCreditCheckout: input =>
        Effect.tryPromise({
          catch: error => checkoutError(error),
          try: () => createCreditCheckout(config, stripeClient, input),
        }),
      fulfillCheckoutSession: input =>
        Effect.tryPromise({
          catch: error => checkoutError(error),
          try: () => fulfillCheckoutSession(config, stripeClient, input),
        }),
      createSetupIntent: input =>
        Effect.tryPromise({
          catch: error => checkoutError(error),
          try: () => createSetupIntent(stripeClient, input),
        }),
      saveSetupIntentPaymentMethod: input =>
        Effect.tryPromise({
          catch: error => checkoutError(error),
          try: () => saveSetupIntentPaymentMethod(stripeClient, input),
        }),
      chargeAutoTopUp: input =>
        Effect.tryPromise({
          catch: error => checkoutError(error),
          try: () => chargeAutoTopUp(stripeClient, input),
        }),
    }
  }),
)

export type StripeWebhookServiceShape = Readonly<{
  processWebhook: (input: {
    db: D1Database
    payload: string
    signature: string | null
  }) => Effect.Effect<
    StripeWebhookResult,
    StripeProviderError | StripeWebhookError
  >
}>

export class StripeWebhookService extends Context.Service<
  StripeWebhookService,
  StripeWebhookServiceShape
>()('@openagentsinc/StripeWebhookService') {}

export const StripeWebhookServiceLive = Layer.effect(
  StripeWebhookService,
  Effect.gen(function* () {
    const config = yield* StripeConfig
    const checkout = yield* StripeCheckoutService
    const stripeClient = yield* StripeClient

    return {
      processWebhook: input =>
        Effect.tryPromise({
          catch: error => webhookError(error),
          try: () =>
            processStripeWebhook(config, checkout, stripeClient, input),
        }),
    }
  }),
)

export class BillingCreditService extends Context.Service<
  BillingCreditService,
  Readonly<{
    applyStripeCheckoutCredit: (input: {
      amountCents: number
      bonusCents?: number | undefined
      db: D1Database
      packageId: string
      sessionId: string
      userId: string
    }) => Effect.Effect<BillingSummary, StripeCheckoutError>
  }>
>()('@openagentsinc/BillingCreditService') {
  static Live = Layer.succeed(BillingCreditService, {
    applyStripeCheckoutCredit: input =>
      Effect.tryPromise({
        catch: error => new StripeCheckoutError({ reason: safeReason(error) }),
        try: () => applyStripeCheckoutCredit(input.db, input),
      }),
  })
}

export const StripeBillingServicesLive = Layer.mergeAll(
  StripeWebhookServiceLive.pipe(
    Layer.provide(
      StripeCheckoutServiceLive.pipe(
        Layer.provide(
          StripeCustomerServiceLive.pipe(Layer.provide(StripeClient.Live)),
        ),
      ),
    ),
  ),
  BillingCreditService.Live,
)

const trimmed = (value: string | undefined): string | undefined => {
  const next = value?.trim()

  return next === undefined || next === '' ? undefined : next
}

const safeReason = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const stripeProviderError = (
  operation: string,
  error: unknown,
): StripeProviderError =>
  new StripeProviderError({ operation, reason: safeReason(error) })

const checkoutError = (
  error: unknown,
): StripeCheckoutError | StripeProviderError =>
  error instanceof StripeProviderError || error instanceof StripeCheckoutError
    ? error
    : new StripeCheckoutError({ reason: safeReason(error) })

const webhookError = (
  error: unknown,
): StripeWebhookError | StripeProviderError =>
  error instanceof StripeProviderError || error instanceof StripeWebhookError
    ? error
    : new StripeWebhookError({ reason: safeReason(error) })

const requireConfig = (
  env: StripeBillingEnv,
  field: keyof StripeBillingEnv,
): Effect.Effect<string, StripeConfigError> => {
  const value = trimmed(env[field])

  return value === undefined
    ? Effect.fail(
        new StripeConfigError({
          field,
          reason: 'Required Stripe configuration is missing.',
        }),
      )
    : Effect.succeed(value)
}

const redactedStripeSecret = (
  field: keyof StripeBillingEnv,
  value: string,
): Redacted.Redacted<WorkerSecret> =>
  Redacted.make(WorkerSecret.make(value), { label: field })

const readPackages = (
  value: string,
): Effect.Effect<
  ReadonlyMap<BillingCreditPackageId, StripeCreditPackage>,
  StripeConfigError
> =>
  Effect.gen(function* () {
    const raw = yield* Effect.try({
      catch: error =>
        new StripeConfigError({
          field: 'STRIPE_CREDIT_PACKAGES_JSON',
          reason: safeReason(error),
        }),
      try: () =>
        parseJsonWithSchema(
          S.Array(
            S.Struct({
              amountCents: S.Number,
              bonusCents: S.optionalKey(S.Number),
              id: S.String,
              label: S.String,
              priceId: S.String,
            }),
          ),
          value,
        ),
    })
    const packages = yield* Effect.forEach(raw, item => {
      const id = item.id.trim()
      const priceId = item.priceId.trim()
      const amountCents = Math.trunc(item.amountCents)
      const bonusCents = Math.trunc(item.bonusCents ?? 0)
      const label = item.label.trim()

      return id === '' ||
        priceId === '' ||
        label === '' ||
        amountCents <= 0 ||
        bonusCents < 0
        ? Effect.fail(
            new StripeConfigError({
              field: 'STRIPE_CREDIT_PACKAGES_JSON',
              reason:
                'Expected package id, label, positive amountCents, non-negative bonusCents, and priceId.',
            }),
          )
        : Effect.succeed({
            amountCents,
            bonusCents,
            totalCreditCents: amountCents + bonusCents,
            currency: 'USD' as const,
            creditsExpire: false as const,
            id: BillingCreditPackageId.make(id),
            label,
            priceId: StripePriceId.make(priceId),
          })
    })

    return new Map(packages.map(item => [item.id, item]))
  })

// Project the server-configured Stripe credit catalog into display-ready items
// for the billing UI. This is catalog-only: it reads `STRIPE_CREDIT_PACKAGES_JSON`
// WITHOUT requiring the Stripe API key or webhook secret, so the billing summary
// keeps working (and simply offers the configured packages) even on an
// environment where card checkout is otherwise unconfigured. Best-effort by
// design: any missing/invalid catalog yields an empty list rather than an error,
// because a billing summary read must never fail on catalog shape. The `id`
// returned is the real catalog id the checkout endpoint accepts, so the UI can
// never send a packageId the server does not recognize.
export const readBillingCreditPackages = (
  env: StripeBillingEnv,
): ReadonlyArray<BillingCreditPackageDisplay> => {
  const packagesJson = trimmed(env.STRIPE_CREDIT_PACKAGES_JSON)

  if (packagesJson === undefined) {
    return []
  }

  return Effect.runSync(
    readPackages(packagesJson).pipe(
      Effect.map(catalog =>
        Array.from(catalog.values()).map(
          (pack): BillingCreditPackageDisplay => ({
            id: pack.id,
            label: pack.label,
            paidAmountCents: pack.amountCents,
            paidAmountFormatted: formatUsdCents(pack.amountCents),
            bonusCents: pack.bonusCents,
            bonusFormatted: formatUsdCents(pack.bonusCents),
            amountCents: pack.totalCreditCents,
            amountFormatted: formatUsdCents(pack.totalCreditCents),
            creditsExpire: false,
            currency: pack.currency,
          }),
        ),
      ),
      Effect.orElseSucceed(() => []),
    ),
  )
}

export const decodeStripeConfig = (
  env: StripeBillingEnv,
): Effect.Effect<StripeConfigShape, StripeConfigError> =>
  Effect.gen(function* () {
    const appUrl = yield* requireConfig(env, 'OPENAGENTS_APP_URL')
    const app = new URL(appUrl)
    const successUrl =
      trimmed(env.STRIPE_CHECKOUT_SUCCESS_URL) ??
      `${app.origin}/api/billing/stripe/checkout-return?session_id={CHECKOUT_SESSION_ID}`
    const cancelUrl =
      trimmed(env.STRIPE_CHECKOUT_CANCEL_URL) ?? `${app.origin}/billing`
    const packagesJson = yield* requireConfig(
      env,
      'STRIPE_CREDIT_PACKAGES_JSON',
    )

    return {
      apiKey: redactedStripeSecret(
        'STRIPE_API_KEY',
        yield* requireConfig(env, 'STRIPE_API_KEY'),
      ),
      apiVersion: STRIPE_API_VERSION,
      cancelUrl,
      packages: yield* readPackages(packagesJson),
      successUrl,
      webhookSigningSecret: redactedStripeSecret(
        'STRIPE_WEBHOOK_SIGNING_SECRET',
        yield* requireConfig(env, 'STRIPE_WEBHOOK_SIGNING_SECRET'),
      ),
    }
  })

const ensureStripeCustomer = async (
  stripeClient: StripeClientShape,
  input: Readonly<{
    db: D1Database
    email?: string | undefined
    userId: string
  }>,
): Promise<StripeCustomerId> => {
  const existing = await input.db
    .prepare(
      `SELECT stripe_customer_id
       FROM stripe_customers
       WHERE user_id = ? AND currency = ? AND livemode = 0`,
    )
    .bind(input.userId, BILLING_CURRENCY)
    .first<Readonly<{ stripe_customer_id: string }>>()

  if (existing !== null) {
    return StripeCustomerId.make(existing.stripe_customer_id)
  }

  const stripe = stripeClient.unsafeClient()
  const customer = await stripe.customers.create({
    ...(input.email === undefined ? {} : { email: input.email }),
    metadata: {
      omega_user_id: input.userId,
      product: 'openagents_autopilot_credits',
    },
  })
  const now = systemBillingRuntime.nowIso()

  await input.db
    .prepare(
      `INSERT INTO stripe_customers
        (user_id, currency, stripe_customer_id, livemode, email_snapshot, created_at, updated_at)
       VALUES (?, ?, ?, 0, ?, ?, ?)`,
    )
    .bind(
      input.userId,
      BILLING_CURRENCY,
      customer.id,
      input.email ?? null,
      now,
      now,
    )
    .run()

  return StripeCustomerId.make(customer.id)
}

const packageForId = (
  config: StripeConfigShape,
  packageId: string,
): StripeCreditPackage => {
  const pack = config.packages.get(BillingCreditPackageId.make(packageId))

  if (pack === undefined) {
    throw new StripeCheckoutError({ reason: 'Unknown credit package.' })
  }

  return pack
}

const createCreditCheckout = async (
  config: StripeConfigShape,
  stripeClient: StripeClientShape,
  input: Readonly<{
    businessKickoff?: StripeBusinessCheckoutKickoffInput | undefined
    db: D1Database
    email?: string | undefined
    packageId: string
    userId: string
  }>,
): Promise<StripeCheckoutSnapshot> => {
  const pack = packageForId(config, input.packageId)
  const customerId = await ensureStripeCustomer(stripeClient, {
    db: input.db,
    email: input.email,
    userId: input.userId,
  })
  const stripe = stripeClient.unsafeClient()
  const session = await stripe.checkout.sessions.create(
    {
      cancel_url: config.cancelUrl,
      client_reference_id: input.userId,
      customer: customerId,
      line_items: [{ price: pack.priceId, quantity: 1 }],
      metadata: {
        amount_cents: String(pack.amountCents),
        bonus_cents: String(pack.bonusCents),
        business_credit_grant_cents: String(pack.amountCents),
        ...(input.businessKickoff === undefined
          ? {}
          : {
              business_setup_fee_cents: '0',
              business_signup_id: input.businessKickoff.signupId,
            }),
        currency: pack.currency,
        credits_expire: 'false',
        omega_user_id: input.userId,
        package_id: pack.id,
        total_credit_cents: String(pack.totalCreditCents),
      },
      mode: 'payment',
      success_url: config.successUrl,
    },
    {
      idempotencyKey: `billing:stripe-checkout-create:${input.userId}:${pack.id}:${systemBillingRuntime.randomId('attempt')}`,
    },
  )
  const now = systemBillingRuntime.nowIso()

  await input.db
    .prepare(
      `INSERT OR IGNORE INTO stripe_checkout_sessions
        (session_id, user_id, package_id, amount_cents, currency, payment_status,
         fulfillment_status, ledger_entry_id, stripe_customer_id, checkout_url, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, 'pending', NULL, ?, ?, ?, ?)`,
    )
    .bind(
      session.id,
      input.userId,
      pack.id,
      pack.amountCents,
      BILLING_CURRENCY,
      session.payment_status ?? 'unpaid',
      customerId,
      session.url ?? null,
      now,
      now,
    )
    .run()

  if (session.url === null) {
    throw new StripeCheckoutError({
      reason: 'Stripe did not return a Checkout URL.',
    })
  }

  return {
    amountCents: pack.amountCents,
    checkoutUrl: session.url,
    currency: 'USD',
    customerId,
    packageId: pack.id,
    paymentStatus: session.payment_status ?? 'unpaid',
    sessionId: StripeCheckoutSessionId.make(session.id),
    userId: input.userId,
  }
}

/**
 * Map a fulfilled Stripe credit checkout onto the partner-payout feed's
 * qualifying-event shape (autopilot_sites.partner_payout_ledger.v1).
 *
 * Pure and deterministic so the Stripe -> partner-rail contract is testable
 * without a live Stripe client or D1:
 *  - `asset: 'usd'` and `qualifyingAmount = amountCents` — the role percentage
 *    is applied to the USD purchase value, so any resulting eligibility is a
 *    USD-asset row that never mints a withdrawable-Bitcoin liability.
 *  - `customerUserId` is the PAYER, who is also recorded as the ledger
 *    beneficiary and drives the self-payout exclusion.
 *  - `idempotencyKey` is deterministic per checkout session, so a webhook
 *    redelivery creates the eligibility row at most once.
 * Whether any partner is actually credited is decided downstream by the
 * no-fallback attribution policy (an EXPLICIT active agreement must cover the
 * customer); the common no-agreement case records nothing.
 */
export const buildStripeCheckoutPartnerPayoutEvent = (
  input: Readonly<{
    amountCents: number
    nowIso: string
    sessionId: string
    userId: string
  }>,
): PartnerQualifyingPaidEvent => ({
  asset: 'usd',
  customerUserId: input.userId,
  eventIso: input.nowIso,
  idempotencyKey: `partner_payout.stripe_checkout.${input.sessionId}`,
  periodKey: input.nowIso.slice(0, 7),
  qualifyingAmount: input.amountCents,
  qualifyingEventKind: 'stripe_credit_purchase',
  qualifyingEventRef: `evidence.stripe_checkout_paid.${input.sessionId}`,
})

const metadataInt = (
  metadata: Stripe.Metadata | null,
  key: string,
): number | undefined => {
  const raw = metadata?.[key]

  if (raw === undefined) {
    return undefined
  }

  const parsed = Number.parseInt(raw, 10)

  return Number.isFinite(parsed) && parsed >= 0 ? parsed : undefined
}

const checkoutCreditGrantCents = (
  session: Stripe.Checkout.Session,
  fallbackAmountCents: number,
): number => {
  const grant = metadataInt(session.metadata, 'business_credit_grant_cents')

  return grant === undefined ? fallbackAmountCents : Math.trunc(grant)
}

const checkoutSetupFeeCents = (
  session: Stripe.Checkout.Session,
  totalAmountCents: number,
  creditGrantCents: number,
): number => {
  const fee = metadataInt(session.metadata, 'business_setup_fee_cents')

  return fee === undefined
    ? Math.max(0, totalAmountCents - creditGrantCents)
    : Math.trunc(fee)
}

const businessSignupIdFromSession = (
  session: Stripe.Checkout.Session,
): string | undefined => {
  const raw = session.metadata?.business_signup_id?.trim()

  return raw === undefined || raw === '' ? undefined : raw
}

const fulfillCheckoutSession = async (
  config: StripeConfigShape,
  stripeClient: StripeClientShape,
  input: Readonly<{ db: D1Database; sessionId: string }>,
): Promise<BillingSummary> => {
  const stripe = stripeClient.unsafeClient()
  const session = await stripe.checkout.sessions.retrieve(input.sessionId)
  const packageId = String(session.metadata?.package_id ?? '')
  const userId = String(
    session.metadata?.omega_user_id ?? session.client_reference_id ?? '',
  )
  const pack = packageForId(config, packageId)
  const now = systemBillingRuntime.nowIso()

  if (userId === '') {
    throw new StripeCheckoutError({
      reason: 'Checkout Session is missing user metadata.',
    })
  }

  if (session.payment_status !== 'paid') {
    await input.db
      .prepare(
        `UPDATE stripe_checkout_sessions
         SET payment_status = ?, fulfillment_status = ?, updated_at = ?
         WHERE session_id = ?`,
      )
      .bind(
        session.payment_status ?? 'unpaid',
        session.status === 'expired' ? 'expired' : 'unpaid',
        now,
        input.sessionId,
      )
      .run()

    return readBillingSummary(input.db, userId)
  }

  const creditGrantCents = checkoutCreditGrantCents(session, pack.amountCents)
  const setupFeeCents = checkoutSetupFeeCents(
    session,
    pack.amountCents,
    creditGrantCents,
  )

  if (setupFeeCents + creditGrantCents !== pack.amountCents) {
    throw new StripeCheckoutError({
      reason:
        'Checkout setup-fee and credit-grant metadata do not match the package total.',
    })
  }

  const summary = await applyStripeCheckoutCredit(input.db, {
    amountCents: creditGrantCents,
    bonusCents: pack.bonusCents,
    packageId: pack.id,
    sessionId: input.sessionId,
    userId,
  })

  const businessSignupId = businessSignupIdFromSession(session)

  if (businessSignupId !== undefined) {
    await provisionBusinessCheckoutKickoff(input.db, {
      checkoutSessionId: input.sessionId,
      creditGrantCents,
      setupFeeCents,
      signupId: businessSignupId,
      totalAmountCents: pack.amountCents,
      userId,
    })
  }

  // RL-1 (openagents #5458): FEED the referral payout ledger from this real
  // paid event. A Stripe credit purchase is USD/credit revenue, so per the
  // rev-share invariant it records a credit-revshare eligibility (qualifying
  // sats = 0 -> the ledger marks it `refused:no_qualifying_paid_amount`), which
  // never creates a withdrawable-Bitcoin liability. The dispatch path
  // (`site-referral-payout-dispatch.ts`) only moves Bitcoin for Bitcoin
  // revenue. This is best-effort: a referral-feed failure must never block
  // billing fulfillment, and the call is idempotent per checkout session.
  try {
    await recordReferralPayoutForPaidEvent(input.db, {
      idempotencyKey: `site_referral_payout.stripe_checkout.${input.sessionId}`,
      nowIso: now,
      periodKey: now.slice(0, 7),
      qualifyingAmountSats: 0,
      qualifyingEventKind: 'stripe_credit_purchase',
      qualifyingEventRef: `evidence.stripe_checkout_paid.${input.sessionId}`,
      revenueAsset: 'usd',
      userId,
    })
  } catch {
    // Swallow: referral feed is non-authoritative for billing fulfillment.
  }

  // autopilot_sites.partner_payout_ledger.v1: FEED the partner payout ledger
  // from the SAME real paid event. Distinct from the referral rail above, this
  // credits a partner ONLY if `recordPartnerPayoutForPaidEvent` finds an
  // EXPLICIT, currently-active `partner_agreements` row covering this customer
  // (the no-fallback attribution policy); the common case — no agreement —
  // records nothing. The qualifying amount is the USD purchase value, so any
  // eligibility is a USD-asset row whose role percentage never mints a
  // withdrawable-Bitcoin liability. Best-effort and idempotent per checkout
  // session: a partner-feed failure must never block billing fulfillment, and
  // every row created here stays operator-gated (eligible -> approve/dispatch).
  try {
    await recordPartnerPayoutForPaidEvent(
      input.db,
      buildStripeCheckoutPartnerPayoutEvent({
        amountCents: pack.amountCents,
        nowIso: now,
        sessionId: input.sessionId,
        userId,
      }),
    )
  } catch {
    // Swallow: partner feed is non-authoritative for billing fulfillment.
  }

  await input.db
    .prepare(
      `UPDATE stripe_checkout_sessions
       SET payment_status = 'paid', fulfillment_status = 'fulfilled',
           ledger_entry_id = COALESCE(
             ledger_entry_id,
             (SELECT id FROM billing_ledger_entries WHERE idempotency_key = ?)
           ),
           updated_at = ?
       WHERE session_id = ?`,
    )
    .bind(`billing:stripe-checkout:${input.sessionId}`, now, input.sessionId)
    .run()

  return summary
}

const createSetupIntent = async (
  stripeClient: StripeClientShape,
  input: Readonly<{
    db: D1Database
    email?: string | undefined
    userId: string
  }>,
): Promise<StripeSetupIntentSnapshot> => {
  const customerId = await ensureStripeCustomer(stripeClient, {
    db: input.db,
    email: input.email,
    userId: input.userId,
  })
  const stripe = stripeClient.unsafeClient()
  const setupIntent = await stripe.setupIntents.create(
    {
      customer: customerId,
      metadata: {
        omega_user_id: input.userId,
        product: 'openagents_autopilot_credits',
      },
      usage: 'off_session',
    },
    {
      idempotencyKey: `billing:stripe-setup-intent:${input.userId}:${systemBillingRuntime.randomId('attempt')}`,
    },
  )

  if (setupIntent.client_secret === null) {
    throw new StripeCheckoutError({
      reason: 'Stripe did not return a SetupIntent client secret.',
    })
  }

  return {
    clientSecret: setupIntent.client_secret,
    customerId,
    setupIntentId: StripeSetupIntentId.make(setupIntent.id),
    status: setupIntent.status,
  }
}

const paymentMethodCardSnapshot = (
  paymentMethod: Stripe.PaymentMethod,
): Readonly<{
  brand: string | null
  expMonth: number | null
  expYear: number | null
  last4: string | null
}> => ({
  brand: paymentMethod.card?.brand ?? null,
  expMonth: paymentMethod.card?.exp_month ?? null,
  expYear: paymentMethod.card?.exp_year ?? null,
  last4: paymentMethod.card?.last4 ?? null,
})

const setupIntentPaymentMethodId = (
  setupIntent: Stripe.SetupIntent,
): string | undefined => {
  const value = setupIntent.payment_method

  return typeof value === 'string' ? value : value?.id
}

const saveSetupIntentPaymentMethod = async (
  stripeClient: StripeClientShape,
  input: Readonly<{
    db: D1Database
    setupIntentId: string
    userId: string
  }>,
): Promise<StripeSavedPaymentMethodSnapshot> => {
  const stripe = stripeClient.unsafeClient()
  const setupIntent = await stripe.setupIntents.retrieve(input.setupIntentId, {
    expand: ['payment_method'],
  })

  if (setupIntent.status !== 'succeeded') {
    throw new StripeCheckoutError({
      reason: 'SetupIntent has not succeeded.',
    })
  }

  const paymentMethodId = setupIntentPaymentMethodId(setupIntent)
  const customerId =
    typeof setupIntent.customer === 'string'
      ? setupIntent.customer
      : setupIntent.customer?.id

  if (paymentMethodId === undefined || customerId === undefined) {
    throw new StripeCheckoutError({
      reason: 'SetupIntent is missing payment method or customer.',
    })
  }

  const rawPaymentMethod =
    typeof setupIntent.payment_method === 'string'
      ? await stripe.paymentMethods.retrieve(paymentMethodId)
      : setupIntent.payment_method
  const paymentMethod = rawPaymentMethod as Stripe.PaymentMethod
  const card = paymentMethodCardSnapshot(paymentMethod)
  const now = systemBillingRuntime.nowIso()

  await stripe.customers.update(customerId, {
    invoice_settings: { default_payment_method: paymentMethodId },
  })
  await input.db
    .prepare(
      `INSERT INTO stripe_saved_payment_methods
        (user_id, currency, livemode, stripe_customer_id,
         stripe_payment_method_id, setup_intent_id, brand, last4, exp_month,
         exp_year, status, created_at, updated_at)
       VALUES (?, ?, 0, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)
       ON CONFLICT(user_id, currency, livemode) DO UPDATE SET
         stripe_customer_id = excluded.stripe_customer_id,
         stripe_payment_method_id = excluded.stripe_payment_method_id,
         setup_intent_id = excluded.setup_intent_id,
         brand = excluded.brand,
         last4 = excluded.last4,
         exp_month = excluded.exp_month,
         exp_year = excluded.exp_year,
         status = 'active',
         updated_at = excluded.updated_at`,
    )
    .bind(
      input.userId,
      BILLING_CURRENCY,
      customerId,
      paymentMethodId,
      setupIntent.id,
      card.brand,
      card.last4,
      card.expMonth,
      card.expYear,
      now,
      now,
    )
    .run()

  return {
    ...card,
    paymentMethodId,
    setupIntentId: StripeSetupIntentId.make(setupIntent.id),
    status: 'active',
  }
}

type AutoTopUpChargeRow = Readonly<{
  amount_cents: number
  enabled: number
  monthly_cap_cents: number
  spent_this_month_cents: number
  status: string
  stripe_customer_id: string | null
  stripe_payment_method_id: string | null
  threshold_cents: number
}>

const chargeAutoTopUp = async (
  stripeClient: StripeClientShape,
  input: Readonly<{
    db: D1Database
    idempotencyKey?: string | undefined
    userId: string
  }>,
): Promise<StripeAutoTopUpChargeResult> => {
  const balanceBeforeCents = await readBillingBalanceCents(
    input.db,
    input.userId,
  )
  const policy = await input.db
    .prepare(
      `SELECT p.enabled, p.threshold_cents, p.amount_cents,
              p.monthly_cap_cents, p.spent_this_month_cents, p.status,
              pm.stripe_customer_id, pm.stripe_payment_method_id
       FROM billing_auto_top_up_policies p
       LEFT JOIN stripe_saved_payment_methods pm
         ON pm.user_id = p.user_id
        AND pm.currency = p.currency
        AND pm.livemode = 0
        AND pm.status = 'active'
       WHERE p.user_id = ? AND p.currency = ?`,
    )
    .bind(input.userId, BILLING_CURRENCY)
    .first<AutoTopUpChargeRow>()

  if (
    policy === null ||
    policy.enabled !== 1 ||
    policy.status !== 'active' ||
    balanceBeforeCents > policy.threshold_cents
  ) {
    return {
      billing: await readBillingSummary(input.db, input.userId),
      message: 'Auto top-up was not needed.',
      status: 'skipped',
    }
  }

  const idempotencyKey =
    input.idempotencyKey ??
    `billing:stripe-auto-top-up:${input.userId}:${systemBillingRuntime.nowIso().slice(0, 10)}:${policy.spent_this_month_cents}`

  if (
    policy.spent_this_month_cents + policy.amount_cents >
    policy.monthly_cap_cents
  ) {
    await recordBillingAutoTopUpEvent(input.db, {
      amountCents: policy.amount_cents,
      balanceBeforeCents,
      idempotencyKey: `${idempotencyKey}:cap`,
      reason: 'Monthly auto top-up cap reached.',
      status: 'cap_reached',
      userId: input.userId,
    })

    return {
      billing: await readBillingSummary(input.db, input.userId),
      message: 'Monthly auto top-up cap reached.',
      status: 'cap_reached',
    }
  }

  if (
    policy.stripe_customer_id === null ||
    policy.stripe_payment_method_id === null
  ) {
    await pauseBillingAutoTopUpPolicy(input.db, {
      reason: 'No saved payment method.',
      userId: input.userId,
    })
    await recordBillingAutoTopUpEvent(input.db, {
      amountCents: policy.amount_cents,
      balanceBeforeCents,
      idempotencyKey: `${idempotencyKey}:missing-payment-method`,
      reason: 'No saved payment method.',
      status: 'requires_payment_method',
      userId: input.userId,
    })

    return {
      billing: await readBillingSummary(input.db, input.userId),
      message: 'Add a card before enabling auto top-up.',
      status: 'requires_payment_method',
    }
  }

  try {
    const stripe = stripeClient.unsafeClient()
    const paymentIntent = await stripe.paymentIntents.create(
      {
        amount: policy.amount_cents,
        confirm: true,
        currency: BILLING_CURRENCY.toLowerCase(),
        customer: policy.stripe_customer_id,
        metadata: {
          omega_user_id: input.userId,
          product: 'openagents_autopilot_auto_top_up',
        },
        off_session: true,
        payment_method: policy.stripe_payment_method_id,
      },
      { idempotencyKey },
    )

    if (paymentIntent.status !== 'succeeded') {
      await pauseBillingAutoTopUpPolicy(input.db, {
        reason: `Payment requires ${paymentIntent.status}.`,
        userId: input.userId,
      })
      await recordBillingAutoTopUpEvent(input.db, {
        amountCents: policy.amount_cents,
        balanceBeforeCents,
        idempotencyKey: `${idempotencyKey}:requires-action`,
        paymentIntentId: paymentIntent.id,
        reason: `Payment requires ${paymentIntent.status}.`,
        status: 'declined',
        userId: input.userId,
      })

      return {
        billing: await readBillingSummary(input.db, input.userId),
        message: 'Auto top-up payment requires attention.',
        status: 'declined',
      }
    }

    return {
      billing: await applyStripeAutoTopUpCredit(input.db, {
        amountCents: policy.amount_cents,
        balanceBeforeCents,
        idempotencyKey,
        paymentIntentId: paymentIntent.id,
        userId: input.userId,
      }),
      status: 'succeeded',
    }
  } catch (error) {
    await pauseBillingAutoTopUpPolicy(input.db, {
      reason: safeReason(error),
      userId: input.userId,
    })
    await recordBillingAutoTopUpEvent(input.db, {
      amountCents: policy.amount_cents,
      balanceBeforeCents,
      idempotencyKey: `${idempotencyKey}:declined`,
      reason: safeReason(error),
      status: 'declined',
      userId: input.userId,
    })

    return {
      billing: await readBillingSummary(input.db, input.userId),
      message: 'Auto top-up was declined.',
      status: 'declined',
    }
  }
}

const processStripeWebhook = async (
  config: StripeConfigShape,
  checkout: StripeCheckoutServiceShape,
  stripeClient: StripeClientShape,
  input: Readonly<{
    db: D1Database
    payload: string
    signature: string | null
  }>,
): Promise<StripeWebhookResult> => {
  if (input.signature === null) {
    throw new StripeWebhookError({ reason: 'Missing Stripe signature.' })
  }

  const stripe = stripeClient.unsafeClient()
  const event = await stripe.webhooks.constructEventAsync(
    input.payload,
    input.signature,
    redactedValue(config.webhookSigningSecret) ?? '',
    undefined,
    Stripe.createSubtleCryptoProvider(),
  )
  const now = systemBillingRuntime.nowIso()
  const session = event.data.object as Stripe.Checkout.Session
  const maybeSessionId = 'id' in session ? session.id : undefined

  await input.db
    .prepare(
      `INSERT OR IGNORE INTO stripe_webhook_events
        (event_id, type, processing_status, checkout_session_id, received_at, processed_at)
       VALUES (?, ?, 'received', ?, ?, NULL)`,
    )
    .bind(event.id, event.type, maybeSessionId ?? null, now)
    .run()

  if (
    event.type !== 'checkout.session.completed' &&
    event.type !== 'checkout.session.async_payment_succeeded' &&
    event.type !== 'checkout.session.async_payment_failed'
  ) {
    await input.db
      .prepare(
        `UPDATE stripe_webhook_events
         SET processing_status = 'ignored', processed_at = ?
         WHERE event_id = ?`,
      )
      .bind(now, event.id)
      .run()

    return {
      eventId: StripeEventId.make(event.id),
      status: 'ignored',
      type: event.type,
    }
  }

  if (maybeSessionId === undefined) {
    throw new StripeWebhookError({
      reason: 'Stripe event did not include a Checkout Session.',
    })
  }

  await fulfillCheckoutSession(config, stripeClient, {
    db: input.db,
    sessionId: maybeSessionId,
  })
  await input.db
    .prepare(
      `UPDATE stripe_webhook_events
       SET processing_status = 'processed', processed_at = ?
       WHERE event_id = ?`,
    )
    .bind(now, event.id)
    .run()

  return {
    eventId: StripeEventId.make(event.id),
    status: 'processed',
    type: event.type,
  }
}

export const makeStripeCheckoutServiceForRoutes = (env: StripeBillingEnv) => {
  const config = Effect.runSync(
    decodeStripeConfig(env).pipe(
      Effect.match({
        onFailure: error => {
          throw error
        },
        onSuccess: value => value,
      }),
    ),
  )
  const stripeClient: StripeClientShape = {
    client: Effect.sync(
      () =>
        new Stripe(redactedValue(config.apiKey) ?? '', {
          apiVersion: config.apiVersion,
          httpClient: Stripe.createFetchHttpClient(),
          maxNetworkRetries: 2,
          timeout: 20_000,
        }),
    ),
    unsafeClient: () =>
      new Stripe(redactedValue(config.apiKey) ?? '', {
        apiVersion: config.apiVersion,
        httpClient: Stripe.createFetchHttpClient(),
        maxNetworkRetries: 2,
        timeout: 20_000,
      }),
  }
  return {
    createCreditCheckout: (input: {
      businessKickoff?: StripeBusinessCheckoutKickoffInput | undefined
      db: D1Database
      email?: string | undefined
      packageId: string
      userId: string
    }) => createCreditCheckout(config, stripeClient, input),
    fulfillCheckoutSession: (input: { db: D1Database; sessionId: string }) =>
      fulfillCheckoutSession(config, stripeClient, input),
    createSetupIntent: (input: {
      db: D1Database
      email?: string | undefined
      userId: string
    }) => createSetupIntent(stripeClient, input),
    saveSetupIntentPaymentMethod: (input: {
      db: D1Database
      setupIntentId: string
      userId: string
    }) => saveSetupIntentPaymentMethod(stripeClient, input),
    chargeAutoTopUp: (input: {
      db: D1Database
      idempotencyKey?: string | undefined
      userId: string
    }) => chargeAutoTopUp(stripeClient, input),
    processWebhook: (input: {
      db: D1Database
      payload: string
      signature: string | null
    }) => {
      const checkout: StripeCheckoutServiceShape = {
        createCreditCheckout: checkoutInput =>
          Effect.tryPromise({
            catch: error => checkoutError(error),
            try: () =>
              createCreditCheckout(config, stripeClient, checkoutInput),
          }),
        fulfillCheckoutSession: checkoutInput =>
          Effect.tryPromise({
            catch: error => checkoutError(error),
            try: () =>
              fulfillCheckoutSession(config, stripeClient, checkoutInput),
          }),
        createSetupIntent: setupInput =>
          Effect.tryPromise({
            catch: error => checkoutError(error),
            try: () => createSetupIntent(stripeClient, setupInput),
          }),
        saveSetupIntentPaymentMethod: setupInput =>
          Effect.tryPromise({
            catch: error => checkoutError(error),
            try: () => saveSetupIntentPaymentMethod(stripeClient, setupInput),
          }),
        chargeAutoTopUp: chargeInput =>
          Effect.tryPromise({
            catch: error => checkoutError(error),
            try: () => chargeAutoTopUp(stripeClient, chargeInput),
          }),
      }

      return processStripeWebhook(config, checkout, stripeClient, input)
    },
  }
}
