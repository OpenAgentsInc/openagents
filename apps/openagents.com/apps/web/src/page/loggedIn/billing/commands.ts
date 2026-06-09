import { Effect, Schema as S } from 'effect'
import { Command } from 'foldkit'

import {
  BillingCheckoutResponse,
  BillingSummaryResponse,
} from '../../../domain/session'
import { errorMessageFromUnknown, requestJson } from '../commands/api'
import {
  FailedCreateBillingCheckout,
  FailedRedeemBillingCoupon,
  SucceededCreateBillingCheckout,
  SucceededRedeemBillingCoupon,
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
