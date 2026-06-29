import { Effect, Schema as S } from 'effect'
import { Command } from 'foldkit'

import {
  BillingCheckoutResponse,
  BillingSetupIntentResponse,
  BillingSummaryResponse,
} from '../../../domain/session'
import { errorMessageFromUnknown, requestJson } from '../commands/api'
import {
  FailedCreateBillingCheckout,
  FailedPrepareBillingCardSetup,
  FailedRedeemBillingCoupon,
  FailedRunBillingAutoTopUp,
  FailedUpdateBillingAutoTopUpPolicy,
  SucceededCreateBillingCheckout,
  SucceededPrepareBillingCardSetup,
  SucceededRedeemBillingCoupon,
  SucceededRunBillingAutoTopUp,
  SucceededUpdateBillingAutoTopUpPolicy,
} from '../message'

export const RedeemBillingCoupon = Command.define(
  'RedeemBillingCoupon',
  { code: S.String },
  SucceededRedeemBillingCoupon,
  FailedRedeemBillingCoupon,
)(({ code }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({ couponCode: code }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.billing.coupon.redeem',
      request: '/api/billing/coupons/redeem',
      schema: BillingSummaryResponse,
    })

    return SucceededRedeemBillingCoupon({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedRedeemBillingCoupon({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const CreateBillingCheckout = Command.define(
  'CreateBillingCheckout',
  { packageId: S.String },
  SucceededCreateBillingCheckout,
  FailedCreateBillingCheckout,
)(({ packageId }) =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({ packageId }),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.billing.checkout.create',
      request: '/api/billing/checkout',
      schema: BillingCheckoutResponse,
    })

    yield* Effect.sync(() => {
      globalThis.location.assign(response.checkoutUrl)
    })

    return SucceededCreateBillingCheckout({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedCreateBillingCheckout({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const PrepareBillingCardSetup = Command.define(
  'PrepareBillingCardSetup',
  {},
  SucceededPrepareBillingCardSetup,
  FailedPrepareBillingCardSetup,
)(() =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({}),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.billing.card.setup',
      request: '/api/billing/stripe/setup-intents',
      schema: BillingSetupIntentResponse,
    })

    return SucceededPrepareBillingCardSetup({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedPrepareBillingCardSetup({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const UpdateBillingAutoTopUpPolicy = Command.define(
  'UpdateBillingAutoTopUpPolicy',
  {
    amountCents: S.Number,
    enabled: S.Boolean,
    monthlyCapCents: S.Number,
    thresholdCents: S.Number,
  },
  SucceededUpdateBillingAutoTopUpPolicy,
  FailedUpdateBillingAutoTopUpPolicy,
)(input =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify(input),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.billing.autoTopUp.policy',
      request: '/api/billing/auto-top-up-policy',
      schema: BillingSummaryResponse,
    })

    return SucceededUpdateBillingAutoTopUpPolicy({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedUpdateBillingAutoTopUpPolicy({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)

export const RunBillingAutoTopUp = Command.define(
  'RunBillingAutoTopUp',
  {},
  SucceededRunBillingAutoTopUp,
  FailedRunBillingAutoTopUp,
)(() =>
  Effect.gen(function* () {
    const response = yield* requestJson({
      init: {
        body: JSON.stringify({}),
        cache: 'no-store',
        credentials: 'include',
        headers: {
          accept: 'application/json',
          'content-type': 'application/json',
        },
        method: 'POST',
      },
      name: 'loggedIn.billing.autoTopUp.run',
      request: '/api/billing/auto-top-up/run',
      schema: BillingSummaryResponse,
    })

    return SucceededRunBillingAutoTopUp({ response })
  }).pipe(
    Effect.catch(error =>
      Effect.succeed(
        FailedRunBillingAutoTopUp({
          error: errorMessageFromUnknown(error),
        }),
      ),
    ),
  ),
)
