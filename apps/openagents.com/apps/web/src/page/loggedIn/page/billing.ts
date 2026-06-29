import { Match as M } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import {
  ClickedBillingPackage,
  ClickedDisableBillingAutoTopUp,
  ClickedEnableBillingAutoTopUp,
  ClickedPrepareBillingCardSetup,
  ClickedRunBillingAutoTopUp,
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
    case 'BillingPreparingCardSetup':
    case 'BillingSavingAutoTopUpPolicy':
    case 'BillingRunningAutoTopUp':
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
      BillingPreparingCardSetup: () => 'Preparing secure card setup...',
      BillingSavingAutoTopUpPolicy: () => 'Saving auto top-up policy...',
      BillingRunningAutoTopUp: () => 'Checking auto top-up...',
      BillingSucceeded: ({ message }) => message,
      BillingFailed: ({ error }) => error,
    }),
  )

const cardLabel = (billing: Model['auth']['billing']): string => {
  const paymentMethod = billing.autoTopUp.savedPaymentMethod

  if (paymentMethod === null) {
    return 'No card saved'
  }

  const brand = paymentMethod.brand ?? 'Card'
  const last4 = paymentMethod.last4 ?? 'saved'

  return `${brand} ${last4}`
}

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
    // Render purchasable packages from the SERVER catalog
    // (`billing.packages`, projected from `STRIPE_CREDIT_PACKAGES_JSON`) instead
    // of a hardcoded list. The buy button posts the real catalog `id`, so the
    // UI can never send a packageId the `/api/billing/checkout` endpoint does
    // not recognize. An empty catalog (Stripe not configured here) renders no
    // buy buttons rather than stale, unpurchasable options.
    packages: billing.packages.map(pack => ({
      id: pack.id,
      label: pack.label,
      amount: pack.amountFormatted,
      detail: `Adds ${pack.amountFormatted} of credit to your balance.`,
      attrs: [
        h.Type('button'),
        ...(busy ? [h.Disabled(true)] : []),
        h.OnClick(ClickedBillingPackage({ packageId: pack.id })),
      ],
    })),
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
    autoTopUp: {
      amountFormatted: billing.autoTopUp.policy.amountFormatted,
      cardLabel: cardLabel(billing),
      enabled: billing.autoTopUp.policy.enabled,
      events: billing.autoTopUp.events.map(event => ({
        amountFormatted: event.amountFormatted,
        createdAt: event.createdAt,
        id: event.id,
        status: event.status,
      })),
      monthlyCapFormatted: billing.autoTopUp.policy.monthlyCapFormatted,
      pauseReason: billing.autoTopUp.policy.pauseReason,
      spentThisMonthFormatted: billing.autoTopUp.policy.spentThisMonthFormatted,
      status: billing.autoTopUp.policy.status,
      thresholdFormatted: billing.autoTopUp.policy.thresholdFormatted,
      cardSetupAttrs: [
        h.Type('button'),
        ...(busy ? [h.Disabled(true)] : []),
        h.OnClick(ClickedPrepareBillingCardSetup()),
      ],
      enableAttrs: [
        h.Type('button'),
        ...(busy || billing.autoTopUp.policy.enabled ? [h.Disabled(true)] : []),
        h.OnClick(ClickedEnableBillingAutoTopUp()),
      ],
      disableAttrs: [
        h.Type('button'),
        ...(busy || !billing.autoTopUp.policy.enabled
          ? [h.Disabled(true)]
          : []),
        h.OnClick(ClickedDisableBillingAutoTopUp()),
      ],
      runAttrs: [
        h.Type('button'),
        ...(busy ? [h.Disabled(true)] : []),
        h.OnClick(ClickedRunBillingAutoTopUp()),
      ],
    },
  })
}
