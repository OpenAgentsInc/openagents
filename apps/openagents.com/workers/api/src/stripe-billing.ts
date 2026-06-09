import { Effect, Layer, Redacted, Schema as S } from 'effect'
import * as Context from 'effect/Context'
import Stripe from 'stripe'

import {
  BILLING_CURRENCY,
  type BillingSummary,
  applyStripeCheckoutCredit,
  readBillingSummary,
  systemBillingRuntime,
} from './billing'
import { WorkerSecret, redactedValue } from './config'
import { parseJsonWithSchema } from './json-boundary'

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

export const BillingCreditPackageId = S.String.pipe(
  S.brand('BillingCreditPackageId'),
)
export type BillingCreditPackageId = typeof BillingCreditPackageId.Type

export type StripeCreditPackage = Readonly<{
  amountCents: number
  currency: 'USD'
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

export type StripeWebhookResult = Readonly<{
  eventId: StripeEventId
  status: 'processed' | 'ignored'
  type: string
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
>()('@openagents/StripeConfig') {
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
>()('@openagents/StripeClient') {
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
>()('@openagents/StripeCustomerService') {}

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
}>

export class StripeCheckoutService extends Context.Service<
  StripeCheckoutService,
  StripeCheckoutServiceShape
>()('@openagents/StripeCheckoutService') {}

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
>()('@openagents/StripeWebhookService') {}

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
      db: D1Database
      packageId: string
      sessionId: string
      userId: string
    }) => Effect.Effect<BillingSummary, StripeCheckoutError>
  }>
>()('@openagents/BillingCreditService') {
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
      const label = item.label.trim()

      return id === '' || priceId === '' || label === '' || amountCents <= 0
        ? Effect.fail(
            new StripeConfigError({
              field: 'STRIPE_CREDIT_PACKAGES_JSON',
              reason:
                'Expected package id, label, positive amountCents, and priceId.',
            }),
          )
        : Effect.succeed({
            amountCents,
            currency: 'USD' as const,
            id: BillingCreditPackageId.make(id),
            label,
            priceId: StripePriceId.make(priceId),
          })
    })

    return new Map(packages.map(item => [item.id, item]))
  })

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
        currency: pack.currency,
        omega_user_id: input.userId,
        package_id: pack.id,
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

  const summary = await applyStripeCheckoutCredit(input.db, {
    amountCents: pack.amountCents,
    packageId: pack.id,
    sessionId: input.sessionId,
    userId,
  })

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
      db: D1Database
      email?: string | undefined
      packageId: string
      userId: string
    }) => createCreditCheckout(config, stripeClient, input),
    fulfillCheckoutSession: (input: { db: D1Database; sessionId: string }) =>
      fulfillCheckoutSession(config, stripeClient, input),
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
      }

      return processStripeWebhook(config, checkout, stripeClient, input)
    },
  }
}
