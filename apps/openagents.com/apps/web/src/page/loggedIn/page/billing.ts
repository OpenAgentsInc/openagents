import { Match as M } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import {
  ClickedBillingPackage,
  Message,
  SubmittedBillingCoupon,
  UpdatedBillingCouponCode,
} from '../message'
import type { Model } from '../model'

const actionStatus = (model: Model): 'idle' | 'busy' | 'success' | 'error' => {
  switch (model.billingAction._tag) {
    case 'BillingIdle':
      return 'idle'
    case 'BillingRedeemingCoupon':
    case 'BillingOpeningCheckout':
      return 'busy'
    case 'BillingSucceeded':
      return 'success'
    case 'BillingFailed':
      return 'error'
  }
}

const actionMessage = (model: Model): string | undefined =>
  M.value(model.billingAction).pipe(
    M.tagsExhaustive({
      BillingIdle: () => undefined,
      BillingRedeemingCoupon: ({ code }) => `Applying coupon ${code}...`,
      BillingOpeningCheckout: () => 'Opening credit checkout...',
      BillingSucceeded: ({ message }) => message,
      BillingFailed: ({ error }) => error,
    }),
  )

export const view = (model: Model): Html => {
  const h = html<Message>()
  const billing = model.auth.billing
  const message = actionMessage(model)
  const busy = actionStatus(model) === 'busy'

  return Ui.billingCreditsPage<Message>({
    balanceFormatted: billing.balanceFormatted,
    status: billing.status,
    minimumRunCreditFormatted: billing.minimumRunCreditFormatted,
    containerRateLabel: `$${(billing.rates.containerCentsPerMinute / 100).toFixed(2)}/min`,
    codexRateLabel: `$${(billing.rates.codexCentsPerThousandTokens / 100).toFixed(2)}/1k`,
    couponCode: model.billingCouponCode,
    actionStatus: actionStatus(model),
    ...(message === undefined ? {} : { actionMessage: message }),
    couponFormAttrs: [h.OnSubmit(SubmittedBillingCoupon())],
    couponInputAttrs: [h.OnInput(value => UpdatedBillingCouponCode({ value }))],
    packages: [
      {
        id: 'starter',
        label: 'Starter',
        amount: '$25',
        detail: 'Light testing and short computer turns.',
        attrs: [
          h.Type('button'),
          ...(busy ? [h.Disabled(true)] : []),
          h.OnClick(ClickedBillingPackage({ packageId: 'starter' })),
        ],
      },
      {
        id: 'builder',
        label: 'Builder',
        amount: '$100',
        detail: 'Frequent repo work and longer Autopilot runs.',
        attrs: [
          h.Type('button'),
          ...(busy ? [h.Disabled(true)] : []),
          h.OnClick(ClickedBillingPackage({ packageId: 'builder' })),
        ],
      },
      {
        id: 'team',
        label: 'Team',
        amount: '$500',
        detail: 'Shared team usage across sustained workrooms.',
        attrs: [
          h.Type('button'),
          ...(busy ? [h.Disabled(true)] : []),
          h.OnClick(ClickedBillingPackage({ packageId: 'team' })),
        ],
      },
    ],
    recentEntries: billing.recentEntries.map(entry => ({
      id: entry.id,
      description: entry.description,
      amountFormatted: entry.amountFormatted,
      source: entry.source,
      createdAt: entry.createdAt,
    })),
    activeRuns: billing.activeRuns.map(run => ({
      id: run.id,
      title: run.title,
      status: run.status,
      accruedSeconds: run.accruedSeconds,
      estimatedDebitFormatted: run.estimatedDebitFormatted,
    })),
  })
}
