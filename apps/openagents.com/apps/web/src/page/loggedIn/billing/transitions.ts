import { Match as M, Option } from 'effect'
import { evo } from 'foldkit/struct'

import { type BillingSummary } from '../../../domain/session'
import { Message } from '../message'
import {
  FailedBillingAction,
  IdleBillingAction,
  Model,
  OpeningBillingCheckout,
  RedeemingBillingCoupon,
  SucceededBillingAction,
  authWithBilling,
  sidebarWithBilling,
} from '../model'
import { type UpdateReturn, noUpdate } from '../transition'
import { CreateBillingCheckout, RedeemBillingCoupon } from './commands'

const withUpdateReturn = M.withReturnType<UpdateReturn>()

const applyBillingSummary = (model: Model, billing: BillingSummary): Model => {
  const auth = authWithBilling(model.auth, billing)

  return evo(model, {
    auth: () => auth,
    sidebar: sidebar => sidebarWithBilling(sidebar, billing),
  })
}

export const updateBilling = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tags({
      ClickedBillingPackage: ({ packageId }) => [
        evo(model, {
          billingAction: () => OpeningBillingCheckout({ packageId }),
        }),
        [CreateBillingCheckout({ packageId })],
        Option.none(),
      ],
      UpdatedBillingCouponCode: ({ value }) => [
        evo(model, {
          billingAction: () => IdleBillingAction(),
          billingCouponCode: () => value,
        }),
        [],
        Option.none(),
      ],
      SubmittedBillingCoupon: () => {
        const code = model.billingCouponCode.trim()

        if (code === '') {
          return [
            evo(model, {
              billingAction: () =>
                FailedBillingAction({ error: 'Enter a coupon code.' }),
            }),
            [],
            Option.none(),
          ]
        }

        return [
          evo(model, {
            billingAction: () => RedeemingBillingCoupon({ code }),
          }),
          [RedeemBillingCoupon({ code })],
          Option.none(),
        ]
      },
      SucceededRedeemBillingCoupon: ({ response }) => [
        evo(applyBillingSummary(model, response.billing), {
          billingAction: () =>
            SucceededBillingAction({
              message: response.message ?? 'Credits applied.',
            }),
          billingCouponCode: () => '',
        }),
        [],
        Option.none(),
      ],
      FailedRedeemBillingCoupon: ({ error }) => [
        evo(model, {
          billingAction: () => FailedBillingAction({ error }),
        }),
        [],
        Option.none(),
      ],
      SucceededCreateBillingCheckout: ({ response }) => [
        evo(applyBillingSummary(model, response.billing), {
          billingAction: () =>
            SucceededBillingAction({
              message: response.message,
            }),
        }),
        [],
        Option.none(),
      ],
      FailedCreateBillingCheckout: ({ error }) => [
        evo(model, {
          billingAction: () => FailedBillingAction({ error }),
        }),
        [],
        Option.none(),
      ],
    }),
    M.orElse(() => noUpdate(model)),
  )
